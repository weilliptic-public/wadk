package weiladk

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// PluginOption configures NewPlugin.
type PluginOption func(*pluginOptions)

type pluginOptions struct {
	actions      []Action
	fallback     IdentityResolver
	dispatcher   *Dispatcher
	flush        bool
	flushTimeout time.Duration
	onError      func(ev *Event, action string, err error)
	tracing      *TraceConfig
	secureAudit  bool // audit secured tool calls (WithSecureA2A)
}

// WithActions adds declarative Actions to the plugin. On each hook, the
// plugin.Config callback runs first; if it does not short-circuit, the
// actions for that hook run in order.
func WithActions(actions ...Action) PluginOption {
	return func(o *pluginOptions) { o.actions = append(o.actions, actions...) }
}

// WithFallbackIdentity picks the identity for agents that have none bound
// (see Bind) — e.g. a workflow agent that only coordinates others.
func WithFallbackIdentity(resolve IdentityResolver) PluginOption {
	return func(o *pluginOptions) { o.fallback = resolve }
}

// WithDispatcher runs the plugin's NonBlocking actions, and Submit calls made
// from its callbacks, on d instead of DefaultDispatcher.
func WithDispatcher(d *Dispatcher) PluginOption {
	return func(o *pluginOptions) { o.dispatcher = d }
}

// WithFlushAfterRun makes the end of each run wait, up to timeout, until
// queued calls have landed. The dispatcher is shared, so this also waits for
// calls queued by concurrent runs.
func WithFlushAfterRun(timeout time.Duration) PluginOption {
	return func(o *pluginOptions) { o.flush, o.flushTimeout = true, timeout }
}

// WithErrorHandler receives failures that do not fail a hook: FailOpen
// blocking actions, NonBlocking actions, and after-run actions. Default logs
// with slog.
func WithErrorHandler(f func(ev *Event, action string, err error)) PluginOption {
	return func(o *pluginOptions) { o.onError = f }
}

// NewPlugin is plugin.New with on-chain identity: the same plugin.Config —
// your ADK callbacks, unchanged — where weiladk.Invoke, weiladk.Submit and
// weiladk.IdentityOf act as the agent the callback runs for. Options add
// declarative Actions and control the background queue.
//
// Register it in runner.Config.PluginConfig, first: ADK stops at the first
// plugin callback that returns a value.
func NewPlugin(cfg plugin.Config, opts ...PluginOption) (*plugin.Plugin, error) {
	p, err := newWeilPlugin(cfg, opts...)
	if err != nil {
		return nil, err
	}
	pl, err := plugin.New(p.config())
	if err != nil {
		return nil, err
	}
	p.self = pl // passed on to agents run as tools (NewAgentTool)
	return pl, nil
}

func newWeilPlugin(cfg plugin.Config, opts ...PluginOption) (*weilPlugin, error) {
	o := pluginOptions{flushTimeout: 30 * time.Second}
	for _, opt := range opts {
		opt(&o)
	}
	if o.dispatcher == nil {
		o.dispatcher = DefaultDispatcher()
	}
	if o.onError == nil {
		o.onError = func(ev *Event, action string, err error) {
			slog.Warn("weiladk action failed", "action", action, "hook", ev.Hook,
				"agent", ev.AgentName, "invocation", ev.InvocationID, "error", err)
		}
	}
	if cfg.Name == "" {
		cfg.Name = "weil"
	}

	p := &weilPlugin{user: cfg, opts: o, byHook: map[Hook][]*Action{}}
	if o.tracing != nil {
		p.trace = &tracer{cfg: o.tracing, p: p}
	}
	for i := range p.opts.actions {
		a := &p.opts.actions[i]
		if a.Build == nil {
			return nil, fmt.Errorf("weiladk: action %q has no Build", a.label())
		}
		if len(a.Hooks) == 0 {
			return nil, fmt.Errorf("weiladk: action %q has no Hooks", a.label())
		}
		for _, h := range a.Hooks {
			p.byHook[h] = append(p.byHook[h], a)
		}
	}
	return p, nil
}

type weilPlugin struct {
	// finished holds tool calls that have run, by function call ID, until
	// their function response event — what the model receives, after every
	// callback — is traced and audited (see onToolResponses).
	finished sync.Map // function call ID → finishedCall
	user     plugin.Config
	opts     pluginOptions
	byHook   map[Hook][]*Action
	trace    *tracer // nil unless WithTracing
	self     *plugin.Plugin
	// durable counts each run's queued calls with their own retry policy
	// (audit entries), which the run waits for before it ends.
	durable sync.Map // invocation ID → *runDurable
}

// runDurable counts a run's queued durable calls. An entry lives in
// weilPlugin.durable only while it has calls pending: it is created by the
// first, removed by the last to finish, or taken out by AfterRun to wait on.
// A run resumed under the same invocation ID (an A2A human-in-the-loop
// reply) or a hook firing after AfterRun therefore starts a fresh entry.
type runDurable struct {
	mu      sync.Mutex
	n       int
	gone    bool          // out of the map: take no more calls
	waiting bool          // AfterRun waits on done
	done    chan struct{} // closed when n reaches 0 while waiting
}

// trackDurable counts a call queued for the run invocationID and returns
// the func that marks it done.
func (p *weilPlugin) trackDurable(invocationID string) func() {
	for {
		v, _ := p.durable.LoadOrStore(invocationID, &runDurable{})
		r := v.(*runDurable)
		r.mu.Lock()
		if r.gone {
			r.mu.Unlock()
			continue // removed meanwhile: start a fresh entry
		}
		r.n++
		r.mu.Unlock()

		var once sync.Once
		return func() {
			once.Do(func() {
				r.mu.Lock()
				defer r.mu.Unlock()
				if r.n--; r.n > 0 {
					return
				}
				if r.waiting {
					close(r.done)
					return
				}
				r.gone = true
				p.durable.CompareAndDelete(invocationID, r)
			})
		}
	}
}

// awaitDurable waits until the run's queued audit entries have been
// accepted (or have failed for good), so a run never ends with its audit
// trail still in memory — as the Python SDK awaits each entry. Under heavy
// load this wait is the platform's queue; past SignedTxnTTL the run ends and
// the entries carry on in the background.
func (p *weilPlugin) awaitDurable(ev *Event) {
	v, ok := p.durable.LoadAndDelete(ev.InvocationID)
	if !ok {
		return // nothing pending
	}
	r := v.(*runDurable)
	r.mu.Lock()
	r.gone = true
	if r.n == 0 {
		r.mu.Unlock()
		return
	}
	r.waiting = true
	r.done = make(chan struct{})
	done := r.done
	r.mu.Unlock()

	select {
	case <-done:
	case <-time.After(client.SignedTxnTTL):
		r.mu.Lock()
		pending := r.n
		r.mu.Unlock()
		p.opts.onError(ev, "audit", fmt.Errorf("weiladk: %d audit entries not yet accepted when the run ended; they are still being submitted", pending))
	}
}

func (p *weilPlugin) has(h Hook) bool { return len(p.byHook[h]) > 0 }

// agentCtx and invCtx expose the plugin's dispatcher to Submit calls made
// from user callbacks. ADK's callback contexts cannot be re-parented
// (WithAgentContext is unsupported there), so the context is wrapped: every
// method is ADK's own, except that Value also answers the dispatcher key.
func (p *weilPlugin) agentCtx(ctx agent.Context) agent.Context {
	return agentCtxWithDispatcher{Context: ctx, d: p.opts.dispatcher}
}

func (p *weilPlugin) invCtx(ctx agent.InvocationContext) agent.InvocationContext {
	return invCtxWithDispatcher{InvocationContext: ctx, d: p.opts.dispatcher}
}

type agentCtxWithDispatcher struct {
	agent.Context
	d *Dispatcher
}

func (c agentCtxWithDispatcher) Value(key any) any {
	if key == (dispatcherKey{}) {
		return c.d
	}
	return c.Context.Value(key)
}

type invCtxWithDispatcher struct {
	agent.InvocationContext
	d *Dispatcher
}

func (c invCtxWithDispatcher) Value(key any) any {
	if key == (dispatcherKey{}) {
		return c.d
	}
	return c.InvocationContext.Value(key)
}

// config wraps every hook that has a user callback or an action.
func (p *weilPlugin) config() plugin.Config {
	u := p.user
	c := plugin.Config{Name: u.Name, CloseFunc: p.close}

	if u.OnUserMessageCallback != nil || p.has(HookUserMessage) {
		c.OnUserMessageCallback = func(ic agent.InvocationContext, content *genai.Content) (*genai.Content, error) {
			ic = p.invCtx(ic)
			if u.OnUserMessageCallback != nil {
				if out, err := u.OnUserMessageCallback(ic, content); out != nil || err != nil {
					return out, err
				}
			}
			ev := eventFromInvocationContext(HookUserMessage, ic)
			ev.UserContent = content
			return nil, p.fire(ev)
		}
	}
	// Always track the run's root agent, so bound identities resolve even
	// when agents in different trees share a name.
	{
		c.BeforeRunCallback = func(ic agent.InvocationContext) (*genai.Content, error) {
			registerRunPlugin(ic.InvocationID(), p.self)
			if a := ic.Agent(); a != nil {
				runRoots.Store(ic.InvocationID(), a)
				rememberTree(a)
			}
			ic = p.invCtx(ic)
			if p.trace != nil {
				p.trace.begin(eventFromInvocationContext(HookBeforeRun, ic))
			}
			if u.BeforeRunCallback != nil {
				if out, err := u.BeforeRunCallback(ic); out != nil || err != nil {
					return out, err
				}
			}
			if !p.has(HookBeforeRun) {
				return nil, nil
			}
			return nil, p.fire(eventFromInvocationContext(HookBeforeRun, ic))
		}
	}
	{
		c.AfterRunCallback = func(ic agent.InvocationContext) {
			defer runRoots.Delete(ic.InvocationID())
			defer unregisterRun(ic.InvocationID())
			ic = p.invCtx(ic)
			if u.AfterRunCallback != nil {
				u.AfterRunCallback(ic)
			}
			ev := eventFromInvocationContext(HookAfterRun, ic)
			if err := p.fire(ev); err != nil {
				p.opts.onError(ev, "after_run", err)
			}
			p.flushFinished(ic.InvocationID())
			if p.trace != nil {
				p.trace.finish(ev)
			}
			p.awaitDurable(ev)
			if !p.opts.flush {
				return
			}
			ctx, cancel := context.WithTimeout(context.WithoutCancel(ic), p.opts.flushTimeout)
			defer cancel()
			if err := p.opts.dispatcher.Flush(ctx); err != nil {
				p.opts.onError(ev, "flush", err)
			}
		}
	}
	if u.OnEventCallback != nil || p.has(HookEvent) || p.trace != nil || p.opts.secureAudit {
		c.OnEventCallback = func(ic agent.InvocationContext, e *session.Event) (*session.Event, error) {
			ic = p.invCtx(ic)
			p.onToolResponses(e)
			if u.OnEventCallback != nil {
				if out, err := u.OnEventCallback(ic, e); out != nil || err != nil {
					return out, err
				}
			}
			ev := eventFromInvocationContext(HookEvent, ic)
			ev.SessionEvent = e
			if e != nil && e.Author != "" {
				ev.AgentName = e.Author
			}
			return nil, p.fire(ev)
		}
	}
	if u.BeforeAgentCallback != nil || p.has(HookBeforeAgent) {
		c.BeforeAgentCallback = func(ctx agent.Context) (*genai.Content, error) {
			ctx = p.agentCtx(ctx)
			if u.BeforeAgentCallback != nil {
				if out, err := u.BeforeAgentCallback(ctx); out != nil || err != nil {
					return out, err
				}
			}
			return nil, p.fire(eventFromAgentContext(HookBeforeAgent, ctx))
		}
	}
	if u.AfterAgentCallback != nil || p.has(HookAfterAgent) {
		c.AfterAgentCallback = func(ctx agent.Context) (*genai.Content, error) {
			ctx = p.agentCtx(ctx)
			if u.AfterAgentCallback != nil {
				if out, err := u.AfterAgentCallback(ctx); out != nil || err != nil {
					return out, err
				}
			}
			return nil, p.fire(eventFromAgentContext(HookAfterAgent, ctx))
		}
	}
	if u.BeforeModelCallback != nil || p.has(HookBeforeModel) {
		c.BeforeModelCallback = func(ctx agent.Context, req *model.LLMRequest) (*model.LLMResponse, error) {
			ctx = p.agentCtx(ctx)
			if u.BeforeModelCallback != nil {
				if out, err := u.BeforeModelCallback(ctx, req); out != nil || err != nil {
					return out, err
				}
			}
			ev := eventFromAgentContext(HookBeforeModel, ctx)
			ev.LLMRequest = req
			return nil, p.fire(ev)
		}
	}
	if u.AfterModelCallback != nil || p.has(HookAfterModel) || p.trace != nil {
		c.AfterModelCallback = func(ctx agent.Context, resp *model.LLMResponse, respErr error) (*model.LLMResponse, error) {
			ctx = p.agentCtx(ctx)
			ev := eventFromAgentContext(HookAfterModel, ctx)
			ev.LLMResponse, ev.Err = resp, respErr
			if u.AfterModelCallback != nil {
				if out, err := u.AfterModelCallback(ctx, resp, respErr); out != nil || err != nil {
					if p.trace != nil && out != nil {
						p.trace.plan(ev, out)
					}
					return out, err
				}
			}
			if p.trace != nil {
				p.trace.plan(ev, resp)
			}
			if !p.has(HookAfterModel) {
				return nil, nil
			}
			return nil, p.fire(ev)
		}
	}
	if u.OnModelErrorCallback != nil || p.has(HookModelError) {
		c.OnModelErrorCallback = func(ctx agent.Context, req *model.LLMRequest, respErr error) (*model.LLMResponse, error) {
			ctx = p.agentCtx(ctx)
			if u.OnModelErrorCallback != nil {
				if out, err := u.OnModelErrorCallback(ctx, req, respErr); out != nil || err != nil {
					return out, err
				}
			}
			ev := eventFromAgentContext(HookModelError, ctx)
			ev.LLMRequest, ev.Err = req, respErr
			return nil, p.fire(ev)
		}
	}
	if u.BeforeToolCallback != nil || p.has(HookBeforeTool) || p.opts.secureAudit {
		c.BeforeToolCallback = func(ctx agent.Context, t tool.Tool, args map[string]any) (map[string]any, error) {
			ctx = p.agentCtx(ctx)
			if p.opts.secureAudit && ctx.FunctionCallID() != "" {
				toolAudits.Store(ctx.FunctionCallID(), pendingToolAudit{p: p, ev: toolEvent(HookBeforeTool, ctx, t, args)})
			}
			if u.BeforeToolCallback != nil {
				if out, err := u.BeforeToolCallback(ctx, t, args); out != nil || err != nil {
					return out, err
				}
			}
			if !p.has(HookBeforeTool) {
				return nil, nil
			}
			return nil, p.fire(toolEvent(HookBeforeTool, ctx, t, args))
		}
	}
	if u.AfterToolCallback != nil || p.has(HookAfterTool) || p.trace != nil || p.opts.secureAudit {
		c.AfterToolCallback = func(ctx agent.Context, t tool.Tool, args, result map[string]any, toolErr error) (map[string]any, error) {
			ctx = p.agentCtx(ctx)
			ev := toolEvent(HookAfterTool, ctx, t, args)
			ev.ToolResult, ev.Err = result, toolErr
			if u.AfterToolCallback != nil {
				if out, err := u.AfterToolCallback(ctx, t, args, result, toolErr); out != nil || err != nil {
					p.afterTool(ev, modelSees(out, err), err == nil)
					return out, err
				}
			}
			p.afterTool(ev, modelSees(result, toolErr), toolErr == nil)
			if !p.has(HookAfterTool) {
				return nil, nil
			}
			return nil, p.fire(ev)
		}
	}
	if u.OnToolErrorCallback != nil || p.has(HookToolError) {
		c.OnToolErrorCallback = func(ctx agent.Context, t tool.Tool, args map[string]any, toolErr error) (map[string]any, error) {
			ctx = p.agentCtx(ctx)
			if u.OnToolErrorCallback != nil {
				if out, err := u.OnToolErrorCallback(ctx, t, args, toolErr); out != nil || err != nil {
					return out, err
				}
			}
			ev := toolEvent(HookToolError, ctx, t, args)
			ev.Err = toolErr
			return nil, p.fire(ev)
		}
	}
	return c
}

// finishedCall is a tool call that has run: its after-tool event, the
// response the plugin's callback saw, and whether it succeeded.
type finishedCall struct {
	ev    *Event
	final map[string]any
	ok    bool
}

// afterTool notes a finished tool call. It is traced and audited from its
// function response event (onToolResponses), so a replacement by any
// after-tool callback — the agent's own run after the plugin's in ADK-Go —
// is what gets recorded, as in Python. A call that failed, and that no
// callback replaced, is not traced and audits only adk.tool.before: Python's
// ADK runs no after-tool callback on an exception.
func (p *weilPlugin) afterTool(ev *Event, final map[string]any, ok bool) {
	toolAudits.Delete(ev.FunctionCallID)
	if p.trace == nil && !p.opts.secureAudit {
		takeSecuredCall(ev.FunctionCallID)
		return
	}
	if ev.FunctionCallID == "" {
		p.recordTool(ev, final, ok)
		return
	}
	p.finished.Store(ev.FunctionCallID, finishedCall{ev: ev, final: final, ok: ok})
}

// onToolResponses traces and audits the tool calls whose responses e
// carries, with the response the model receives.
func (p *weilPlugin) onToolResponses(e *session.Event) {
	if e == nil || e.Content == nil {
		return
	}
	for _, part := range e.Content.Parts {
		if part == nil || part.FunctionResponse == nil || part.FunctionResponse.ID == "" {
			continue
		}
		v, ok := p.finished.LoadAndDelete(part.FunctionResponse.ID)
		if !ok {
			continue
		}
		fc := v.(finishedCall)
		final := fc.final
		if fc.ok && part.FunctionResponse.Response != nil {
			final = part.FunctionResponse.Response
		}
		p.recordTool(fc.ev, final, fc.ok)
	}
}

// flushFinished records the run's tool calls whose response event never
// came, with the response the plugin saw.
func (p *weilPlugin) flushFinished(invocationID string) {
	p.finished.Range(func(k, v any) bool {
		if fc := v.(finishedCall); fc.ev.InvocationID == invocationID {
			p.finished.Delete(k)
			p.recordTool(fc.ev, fc.final, fc.ok)
		}
		return true
	})
}

func (p *weilPlugin) recordTool(ev *Event, final map[string]any, ok bool) {
	if p.trace != nil && ok {
		p.trace.record(ev, final)
	}
	if !p.opts.secureAudit {
		takeSecuredCall(ev.FunctionCallID)
		return
	}
	id, err := p.resolveIdentity(nil, ev)
	if err != nil {
		if _, found := takeSecuredCall(ev.FunctionCallID); found {
			p.opts.onError(ev, "tool_audit", err)
		}
		return
	}
	audited := *ev
	audited.Identity = id
	// Inline and in order, as Python awaits each; the calls are fire and
	// forget, so this waits only for the platform to take them.
	for _, call := range toolAuditCalls(&audited, final, ok) {
		if _, err := id.Invoke(context.WithoutCancel(ev.Context()), *call); err != nil {
			p.opts.onError(ev, "tool_audit", err)
		}
	}
}

// modelSees is the function response ADK hands the model for a tool call
// that returned result and err.
func modelSees(result map[string]any, err error) map[string]any {
	if err != nil {
		return map[string]any{"error": err.Error()}
	}
	return result
}

func toolEvent(h Hook, ctx agent.Context, t tool.Tool, args map[string]any) *Event {
	ev := eventFromAgentContext(h, ctx)
	ev.Tool, ev.ToolArgs, ev.FunctionCallID = t, args, ctx.FunctionCallID()
	return ev
}

// fire runs the hook's actions in registration order and returns the first
// error that should fail the hook.
func (p *weilPlugin) fire(ev *Event) error {
	for _, a := range p.byHook[ev.Hook] {
		if a.When != nil && !a.When(ev) {
			continue
		}
		if err := p.runAction(a, ev); err != nil {
			return err
		}
	}
	return nil
}

// resolveIdentity picks who acts for an action: the action's own resolver,
// else the identity bound to the acting agent, else the plugin's fallback.
// a may be nil for the plugin's own calls.
func (p *weilPlugin) resolveIdentity(a *Action, ev *Event) (*Identity, error) {
	if a != nil && a.Identity != nil {
		return a.Identity(ev)
	}
	if id := IdentityOf(ev.Context()); id != nil {
		return id, nil
	}
	if p.opts.fallback != nil {
		return p.opts.fallback(ev)
	}
	return nil, fmt.Errorf("%w (agent %q)", ErrNoIdentity, ev.AgentName)
}

func (p *weilPlugin) runAction(a *Action, shared *Event) error {
	// Each action sees its own copy, so a per-action identity never leaks
	// into another action's view of the event.
	ev := *shared

	fail := func(err error) error {
		if a.Mode == Blocking && a.OnFailure == FailClosed && ev.Hook != HookAfterRun {
			return &ActionError{Action: a.label(), Hook: ev.Hook, Err: err}
		}
		p.opts.onError(&ev, a.label(), err)
		return nil
	}

	id, err := p.resolveIdentity(a, &ev)
	if err != nil {
		return fail(err)
	}
	ev.Identity = id

	call, err := a.Build(&ev)
	if err != nil {
		return fail(fmt.Errorf("build call: %w", err))
	}
	if call == nil {
		return nil
	}

	if a.Mode == Blocking {
		return p.runBlocking(a, &ev, call, fail)
	}
	return p.runNonBlocking(a, &ev, call)
}

func (p *weilPlugin) runBlocking(a *Action, ev *Event, call *Call, fail func(error) error) error {
	ctx := ev.Context()
	if a.Timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, a.Timeout)
		defer cancel()
	}

	res, err := ev.Identity.Invoke(ctx, *call)
	if err != nil {
		return fail(err)
	}
	if a.OnResult != nil {
		if err := a.OnResult(ev, res); err != nil {
			veto := &ActionError{Action: a.label(), Hook: ev.Hook, Err: err}
			if ev.Hook == HookAfterRun {
				p.opts.onError(ev, a.label(), veto)
				return nil
			}
			return veto
		}
	}
	return nil
}

func (p *weilPlugin) runNonBlocking(a *Action, ev *Event, call *Call) error {
	// Encode now: the payload maps belong to ADK and may change once the
	// hook returns.
	args, err := call.encodeArgs()
	if err != nil {
		p.opts.onError(ev, a.label(), fmt.Errorf("encode args: %w", err))
		return nil
	}
	queued := *call
	queued.Args = json.RawMessage(args)

	label := a.label()
	done := func() {}
	if queued.Retry.MaxAttempts > 1 && ev.InvocationID != "" {
		done = p.trackDurable(ev.InvocationID)
	}
	onDone := func(res *Result, err error) {
		defer done()
		if err == nil && a.OnResult != nil {
			err = a.OnResult(ev, res)
		}
		if err != nil {
			p.opts.onError(ev, label, err)
		}
	}
	if err := p.opts.dispatcher.Submit(ev.Context(), ev.Identity, queued, onDone); err != nil {
		done()
		p.opts.onError(ev, label, err)
	}
	return nil
}

// close drains queued calls (the dispatcher may be shared, so it is flushed,
// not closed) and runs the user's CloseFunc.
func (p *weilPlugin) close() error {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	flushErr := p.opts.dispatcher.Flush(ctx)
	if p.user.CloseFunc != nil {
		if err := p.user.CloseFunc(); err != nil {
			return err
		}
	}
	if flushErr != nil {
		return fmt.Errorf("weiladk: queued calls did not drain before close: %w", flushErr)
	}
	return nil
}
