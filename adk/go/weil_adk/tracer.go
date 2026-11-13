package weiladk

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/google/uuid"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"google.golang.org/adk/v2/model"
	"google.golang.org/genai"
)

// ErrTraceNeedsOrg is reported when a run's trace cannot be published because
// its identity is linked to no org and TraceConfig.Publisher is unset (Python:
// "tool tracing requires a wallet linked to an organization").
var ErrTraceNeedsOrg = errors.New("weiladk: tool tracing requires a wallet linked to an organization")

// DefaultTraceTopic is the publisher topic finished traces go to.
const DefaultTraceTopic = "audits"

// Tracer is the systemic tracer applet ("weilchain::weil").
var Tracer = Systemic("weilchain")

// TraceConfig configures WithTracing. Zero values pick the defaults.
type TraceConfig struct {
	// Applet keeping the traces. Default Tracer ("weilchain::weil").
	Applet Applet
	// Publisher receiving each finished trace. Default the publisher of the
	// org the run's identity acts under ("publisher::<org>"), as the Python
	// SDK does; for an identity with no org the trace is then kept but not
	// published, and ErrTraceNeedsOrg is reported.
	Publisher Applet
	// Topic finished traces are published to. Default DefaultTraceTopic.
	Topic string
	// When, if set, filters the tool calls recorded.
	When func(ev *Event) bool
	// SettleTimeout bounds how long the end of a run re-reads get_trace
	// until it holds every recorded event: a read can briefly trail the
	// writes before it. Default 15s.
	SettleTimeout time.Duration
}

// WithTracing records each run as one trace on the tracer applet, as the
// Python SDK's weil_ai tracer does:
//
//   - when the run starts, add_trace(trace_id, root, []), where root is the
//     user's message;
//   - after each tool call, build_trace(trace_id, event), where event is a
//     TraceNode of type "tool_call" whose content is the JSON {"args",
//     "result", "tool"}, with the result the model receives;
//   - when the run ends, get_trace(trace_id), then index_trace(trace_id), and
//     the trace that get_trace returns is published to Topic. get_trace is
//     re-read (up to SettleTimeout) until it holds every recorded event, as
//     a read can trail the writes before it.
//
// The nodes form a graph, as in the Python SDK: each tool call's parent is
// the node recorded last on its ADK branch, so calls made one after another
// form a chain, and calls the model asked for together (in one response) are
// siblings under the same parent. The args of a call carry
// "_weil": {"framework": "adk", "tool_call_id": <ADK function call ID>}.
//
// Every call is signed by the identity resolved for the run's root agent (or
// the fallback identity) when the run starts. The calls queue on the
// plugin's dispatcher in that order, so the agent never waits on them; use
// WithFlushAfterRun to have each run end once they have landed. Failures go
// to the error handler (see WithErrorHandler) and never fail the run. If
// add_trace fails, nothing more is recorded for that run.
//
// Tool calls made inside an agenttool sub-agent are not seen: ADK runs that
// agent in a runner of its own, without this plugin.
func WithTracing(cfg TraceConfig) PluginOption {
	if cfg.Topic == "" {
		cfg.Topic = DefaultTraceTopic
	}
	if cfg.Applet.IsZero() {
		cfg.Applet = Tracer
	}
	if cfg.SettleTimeout <= 0 {
		cfg.SettleTimeout = 15 * time.Second
	}
	return func(o *pluginOptions) { o.tracing = &cfg }
}

// traceRun is one run's trace.
type traceRun struct {
	id        string
	identity  *Identity
	tracer    Applet
	publisher Applet
	// broken is set once add_trace fails; later calls are skipped.
	broken atomic.Bool
	// built counts the events build_trace stored.
	built atomic.Int64

	mu sync.Mutex
	// tips is the last node recorded on each ADK branch.
	tips map[string]string
	// slots holds the node planned for each tool call the model asked for,
	// by function call ID, until the call is recorded.
	slots map[string]traceSlot
}

// traceSlot is a tool call's node: its ID and its parent's ("" for none).
type traceSlot struct{ id, parent string }

// tipLocked returns the last node recorded on branch or, for a branch with
// none yet, on the nearest enclosing branch ("a.b" encloses "a.b.c").
func (r *traceRun) tipLocked(branch string) string {
	for b := branch; ; {
		if tip, ok := r.tips[b]; ok {
			return tip
		}
		if b == "" {
			return ""
		}
		i := strings.LastIndex(b, ".")
		if i < 0 {
			b = ""
		} else {
			b = b[:i]
		}
	}
}

// tracer keeps the traces of the runs in flight, by invocation ID.
type tracer struct {
	cfg  *TraceConfig
	p    *weilPlugin
	runs sync.Map // invocation ID -> *traceRun
}

func (t *tracer) report(ev *Event, err error) {
	t.p.opts.onError(ev, "trace", err)
}

// begin starts a trace for the run ev belongs to.
func (t *tracer) begin(ev *Event) {
	id, err := t.p.resolveIdentity(nil, ev)
	if err != nil {
		t.report(ev, err)
		return
	}
	publisher := t.cfg.Publisher
	if publisher.IsZero() {
		if org := id.Org(); org != nil && org.Name != "" {
			publisher = AppletName("publisher::" + org.Name)
		}
	}
	run := &traceRun{id: uuid.NewString(), identity: id, tracer: t.cfg.Applet, publisher: publisher,
		tips: map[string]string{}, slots: map[string]traceSlot{}}
	t.runs.Store(ev.InvocationID, run)

	args := pyArgs(struct {
		TraceID string   `json:"trace_id"`
		Root    string   `json:"root"`
		Trace   []string `json:"trace"`
	}{run.id, traceRoot(ev.UserContent), []string{}})
	t.submit(ev, run, func(ctx context.Context) error {
		if _, err := t.invoke(ctx, run, Call{Applet: run.tracer, Method: "add_trace", Args: args}); err != nil {
			run.broken.Store(true)
			return err
		}
		return nil
	})
}

// plan reserves a node for each tool call in a model response, under the
// last node of the response's branch: the calls asked for together become
// siblings, whichever of them runs or finishes first. ADK has assigned the
// calls their IDs by the time the response reaches the plugin.
func (t *tracer) plan(ev *Event, resp *model.LLMResponse) {
	if resp == nil || resp.Content == nil {
		return
	}
	v, ok := t.runs.Load(ev.InvocationID)
	if !ok {
		return
	}
	run := v.(*traceRun)
	run.mu.Lock()
	defer run.mu.Unlock()
	parent := run.tipLocked(ev.Branch)
	for _, p := range resp.Content.Parts {
		if p == nil || p.FunctionCall == nil || p.FunctionCall.ID == "" {
			continue
		}
		if _, ok := run.slots[p.FunctionCall.ID]; !ok {
			run.slots[p.FunctionCall.ID] = traceSlot{id: uuid.NewString(), parent: parent}
		}
	}
}

// place returns the node for the tool call ev reports, planned by plan or,
// for a call it did not see, new under the branch's last node; either way the
// node becomes the branch's last.
func (run *traceRun) place(ev *Event) traceSlot {
	run.mu.Lock()
	defer run.mu.Unlock()
	slot, ok := run.slots[ev.FunctionCallID]
	if ok {
		delete(run.slots, ev.FunctionCallID)
	} else {
		slot = traceSlot{id: uuid.NewString(), parent: run.tipLocked(ev.Branch)}
	}
	run.tips[ev.Branch] = slot.id
	return slot
}

// record adds a tool call to the run's trace. result is what the model
// receives for the call.
func (t *tracer) record(ev *Event, result map[string]any) {
	v, ok := t.runs.Load(ev.InvocationID)
	if !ok {
		return
	}
	run := v.(*traceRun)
	if t.cfg.When != nil && !t.cfg.When(ev) {
		// Not recorded, but the call's planned node is no longer needed.
		run.mu.Lock()
		delete(run.slots, ev.FunctionCallID)
		run.mu.Unlock()
		return
	}
	slot := run.place(ev)
	event, err := toolCallNode(slot.id, slot.parent, ev.ToolName(), traceArgs(ev.ToolArgs, ev.FunctionCallID), returnValue(ev, result))
	if err != nil {
		t.report(ev, fmt.Errorf("encode tool call: %w", err))
		return
	}
	args := pyArgs(struct {
		TraceID string    `json:"trace_id"`
		Event   TraceNode `json:"event"`
	}{run.id, event})
	t.submit(ev, run, func(ctx context.Context) error {
		if run.broken.Load() {
			return nil
		}
		if _, err := t.invoke(ctx, run, Call{Applet: run.tracer, Method: "build_trace", Args: args}); err != nil {
			return err
		}
		run.built.Add(1)
		return nil
	})
}

// finish fetches the run's trace, indexes it and publishes it.
func (t *tracer) finish(ev *Event) {
	v, ok := t.runs.LoadAndDelete(ev.InvocationID)
	if !ok {
		return
	}
	run := v.(*traceRun)
	topic := t.cfg.Topic
	report := *ev
	report.Identity = run.identity
	t.submit(ev, run, func(ctx context.Context) error {
		if run.broken.Load() {
			return nil
		}
		trace, err := t.settledTrace(ctx, &report, run)
		if err != nil {
			return err
		}
		// A trace is indexed only once complete. Indexing failing does not
		// keep it from being published. It waits to run, as the publish
		// does: a non-blocking submit can be lost after it is accepted.
		if _, err := t.invoke(ctx, run, Call{Applet: run.tracer, Method: "index_trace",
			Args: pyArgs(traceIDArgs{run.id})}); err != nil {
			t.report(&report, err)
		}
		if run.publisher.IsZero() {
			return ErrTraceNeedsOrg
		}
		// The trace as get_trace returned it, re-encoded as Python's
		// json.dumps does: its keys in the applet's order.
		payload, err := client.PythonDumps(trace, false)
		if err != nil {
			return fmt.Errorf("encode trace: %w", err)
		}
		// Waits for the publish to run. A non-blocking submit only learns that
		// Sentinel took the call: the platform can still drop it before it
		// executes, or the publisher reject it, and nobody would know.
		_, err = t.invoke(ctx, run, Call{Applet: run.publisher, Method: "publish",
			Args: pyArgs(struct {
				Topic   string `json:"topic"`
				Payload string `json:"payload"`
			}{topic, string(payload)})})
		return err
	})
}

// settledTrace reads the run's trace until it holds every event build_trace
// stored, or SettleTimeout passes; then it returns the last read and an
// incomplete trace is reported, not dropped. An applet error is retried the
// same way: straight after add_trace, a read may not see the trace at all
// ("Trace not found").
func (t *tracer) settledTrace(ctx context.Context, ev *Event, run *traceRun) (json.RawMessage, error) {
	want := run.built.Load()
	deadline := time.Now().Add(t.cfg.SettleTimeout)
	backoff := 250 * time.Millisecond
	for {
		res, err := t.invoke(ctx, run, Call{Applet: run.tracer, Method: "get_trace",
			Args: pyArgs(traceIDArgs{run.id})})
		var appletErr *transaction.AppletError
		if errors.As(err, &appletErr) && !time.Now().Add(backoff).After(deadline) {
			time.Sleep(backoff)
			backoff = min(backoff*2, 2*time.Second)
			continue
		}
		if err != nil {
			return nil, err
		}
		var trace json.RawMessage
		if err := res.Decode(&trace); err != nil {
			return nil, fmt.Errorf("get_trace: %w", err)
		}
		var decoded any
		_ = json.Unmarshal(trace, &decoded)
		got := traceLen(decoded)
		if got < 0 || got >= want {
			return trace, nil
		}
		if time.Now().Add(backoff).After(deadline) {
			t.report(ev, fmt.Errorf("trace %s published with %d of %d events: get_trace did not catch up in %s", run.id, got, want, t.cfg.SettleTimeout))
			return trace, nil
		}
		time.Sleep(backoff)
		backoff = min(backoff*2, 2*time.Second)
	}
}

// traceLen counts the events in a get_trace result, or returns -1 if it has
// no "trace" list (so an unknown shape is published as is).
func traceLen(trace any) int64 {
	m, ok := trace.(map[string]any)
	if !ok {
		return -1
	}
	list, ok := m["trace"].([]any)
	if !ok {
		return -1
	}
	return int64(len(list))
}

func (t *tracer) invoke(ctx context.Context, run *traceRun, call Call) (*Result, error) {
	// Trace calls are resent until accepted, like audit entries, so a
	// congested platform delays a trace rather than breaking it. Each is
	// safe to resend: the tracer applet stores an event id once and never
	// lets add_trace replace a stored trace; get_trace and index_trace only
	// read; a resent publish carries the same transaction ID.
	call.Retry = client.DurableRetry
	return t.p.opts.dispatcher.invoke(ctx, run.identity, call)
}

// submit queues fn on the run's lane, so a trace's calls land in order.
func (t *tracer) submit(ev *Event, run *traceRun, fn func(context.Context) error) {
	// Report with a copy: ADK may reuse the hook's payload once it returns.
	report := *ev
	report.Identity = run.identity
	err := t.p.opts.dispatcher.submitOrdered(ev.Context(), "trace:"+run.id, func(ctx context.Context) error {
		if err := fn(ctx); err != nil {
			t.report(&report, err)
			return err
		}
		return nil
	})
	if err != nil {
		t.report(&report, err)
	}
}

type traceIDArgs struct {
	TraceID string `json:"trace_id"`
}

// pyArgs encodes an applet call's arguments as the Python SDK's
// json.dumps(args) does, keys in the struct's order.
func pyArgs(v any) json.RawMessage {
	b, err := client.PythonDumpsValue(v, false)
	if err != nil {
		b, _ = json.Marshal(v)
	}
	return b
}

// TraceNode is one node of a run's trace graph, the TraceEvent record the
// tracer applet's build_trace takes. Content is the node's JSON payload, as
// a string.
type TraceNode struct {
	ID        string  `json:"id"`
	ParentID  *string `json:"parent_id"`
	EventType string  `json:"event_type"`
	Content   string  `json:"content"`
}

// ToolCallNode is the "tool_call" TraceNode for one tool call: id, its
// parent's ID ("" for a root node), and content TraceEvent(tool, args,
// result).
func ToolCallNode(id, parent, tool string, args, result map[string]any) (TraceNode, error) {
	return toolCallNode(id, parent, tool, args, result)
}

func toolCallNode(id, parent, tool string, args map[string]any, result any) (TraceNode, error) {
	b, err := client.PythonDumpsValue(map[string]any{"tool": tool, "args": args, "result": result}, true)
	content := string(b)
	if err != nil {
		return TraceNode{}, err
	}
	n := TraceNode{ID: id, EventType: "tool_call", Content: content}
	if parent != "" {
		n.ParentID = &parent
	}
	return n, nil
}

// returnValue is what the tool returned, as Python's after-tool callback
// receives it: ADK-Go hands a function tool's non-object result to the model
// as {"result": v} (as Python's ADK does after the callbacks), so a tool
// declaring a non-object response is unwrapped back to v.
func returnValue(ev *Event, result map[string]any) any {
	v, ok := result["result"]
	if !ok || len(result) != 1 || ev.Tool == nil {
		return result
	}
	d, ok := ev.Tool.(interface {
		Declaration() *genai.FunctionDeclaration
	})
	if !ok {
		return result
	}
	decl := d.Declaration()
	if decl == nil || decl.ResponseJsonSchema == nil {
		return result
	}
	raw, err := json.Marshal(decl.ResponseJsonSchema)
	if err != nil {
		return result
	}
	var schema struct {
		Type any `json:"type"`
	}
	if json.Unmarshal(raw, &schema) != nil || schema.Type == nil || schema.Type == "object" {
		return result
	}
	return v
}

// traceArgs is a tool call's args as traced: with "_weil" naming the ADK
// function call it was, so a node can be matched to the session's events.
func traceArgs(args map[string]any, functionCallID string) map[string]any {
	if functionCallID == "" {
		return args
	}
	out := make(map[string]any, len(args)+1)
	for k, v := range args {
		out[k] = v
	}
	out["_weil"] = map[string]any{"framework": "adk", "tool_call_id": functionCallID}
	return out
}

// TraceEvent encodes the content of a tool call's node, the JSON object
// {"args", "result", "tool"}, exactly as the Python SDK stores it:
// json.dumps(content, sort_keys=True) — keys sorted at every level, ", " and
// ": " separators, non-ASCII escaped as \uXXXX.
func TraceEvent(tool string, args, result map[string]any) (string, error) {
	b, err := client.PythonDumpsValue(map[string]any{"tool": tool, "args": args, "result": result}, true)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// traceRoot is the text of the message that started the run, or its JSON
// encoding if it has no text.
func traceRoot(c *genai.Content) string {
	if c == nil {
		return ""
	}
	var texts []string
	for _, p := range c.Parts {
		if p != nil && p.Text != "" {
			texts = append(texts, p.Text)
		}
	}
	if len(texts) > 0 {
		return strings.Join(texts, "\n")
	}
	b, _ := json.Marshal(c)
	return string(b)
}
