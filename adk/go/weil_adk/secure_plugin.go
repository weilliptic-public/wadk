package weiladk

import (
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/tool"
)

// SecureA2AConfig configures WithSecureA2A.
type SecureA2AConfig struct {
	// Service is the scope the root agent is served under, recorded on its
	// adk.agent.* audit records.
	Service string
	// DisableAudit stops the adk.agent.* and adk.tool.* audit records, the
	// token usage records and the run traces.
	DisableAudit bool
	// Trace configures the run traces; zero values pick WithTracing's
	// defaults.
	Trace TraceConfig
}

// WithSecureA2A instruments every agent the plugin's runner reaches the way
// the Python SDK's setup_secure_a2a does:
//
//   - adk.agent.before / adk.agent.after for each local agent (remote A2A
//     agents are not audited), signed by that agent's identity, with
//     caller_wallet naming who invoked it and service on the root only;
//     adk.agent.after carries the usage metadata of the agent's model
//     responses that asked for no tool (usage_metadata);
//   - adk.tool.before / adk.tool.after for each SecuredUser / SecuredAdmin
//     tool call, with its arguments and the response the model receives,
//     and for each agent run as a tool (NewAgentTool, agenttool), by
//     metadata only (tool_kind "agent", sub_agent, sub_agent_wallet). A call
//     that fails — e.g. is refused — records only adk.tool.before, as no
//     after-tool callback runs on an exception in the Python SDK's ADK.
//     adk.tool.after carries the usage metadata of the model response that
//     asked for the call;
//   - each model response's token usage, recorded on agent_registry::weil
//     (WeilClient.RecordAgentTokenUsage) once: with the first tool call it
//     asked for, or at the agent's end;
//   - a trace of each run (see WithTracing).
//
// Each audit record is written as its hook fires, before the plugin.Config
// callback for that hook, and is submitted in the background; the end of
// each agent's run waits for every pending entry (FlushAudits, up to
// AuditFlushTimeout), and the end of the run then waits for its trace to be
// published.
//
// NewExecutor installs it on the served side; add it to a calling agent's
// runner for the calling side. Python's setup_secure_a2a also adds the
// Pensieve tools to the root agent: ADK-Go agents cannot gain tools after
// they are built, so add weiladk.PensieveTools() to the root's Tools.
func WithSecureA2A(cfg SecureA2AConfig) PluginOption {
	return func(o *pluginOptions) {
		if cfg.DisableAudit {
			return
		}
		if o.tracing == nil {
			WithTracing(cfg.Trace)(o)
		}
		o.secureAudit, o.secureService = true, cfg.Service
	}
}

// agentStart writes adk.agent.before for the agent run ev reports and opens
// its usage ledger.
func (p *weilPlugin) agentStart(ev *Event) {
	p.usage.open(agentRunKey{ev.InvocationID, ev.AgentName})
	p.writeAgentAudit(ev, nil)
}

// agentEnd ends the agent run ev reports: when its callbacks succeeded,
// adk.agent.after with the usage metadata of its final responses, then
// their token usage on agent_registry; then it waits for the pending audit
// entries.
func (p *weilPlugin) agentEnd(ev *Event, audit bool) {
	usages := p.usage.take(agentRunKey{ev.InvocationID, ev.AgentName})
	if audit {
		var metadata []json.RawMessage
		for _, u := range usages {
			metadata = append(metadata, u.metadata)
		}
		if who := p.writeAgentAudit(ev, metadata); who != nil {
			for _, u := range usages {
				recordTokenUsage(ev.Context(), who, u.tokens)
			}
		}
	}
	flushAuditsFor(func(err error) { p.opts.onError(ev, "audit", err) })
}

// writeAgentAudit submits the adk.agent.* record of ev and returns the
// identity that signed it, or nil if none was written.
func (p *weilPlugin) writeAgentAudit(ev *Event, usageMetadata []json.RawMessage) *Identity {
	who, err := p.resolveIdentity(nil, ev)
	if err != nil {
		p.opts.onError(ev, "agent_audit", err)
		return nil
	}
	e := *ev
	e.Identity = who
	event, fields, ok := agentAuditFields(&e, p.opts.secureService)
	if !ok {
		return nil
	}
	if len(usageMetadata) > 0 {
		fields["usage_metadata"] = usageMetadata
	}
	p.submitAudit(&e, event, fields, ev.Time)
	return who
}

// modelResponse notes a model response the agent receives: its planned
// trace nodes and its token usage.
func (p *weilPlugin) modelResponse(ev *Event, resp *model.LLMResponse) {
	if p.trace != nil {
		p.trace.plan(ev, resp)
	}
	if p.opts.secureAudit {
		p.usage.note(agentRunKey{ev.InvocationID, ev.AgentName}, resp, ev.Time)
	}
}

// toolStart writes adk.tool.before for a secured tool or an agent run as a
// tool. Other tools are left for a secured function wrapped some other way
// to report itself as it starts (auditToolBefore).
func (p *weilPlugin) toolStart(ev *Event) {
	id := ev.FunctionCallID
	if sc, ok := securedToolOf(ev.Tool); ok {
		sc.at, sc.beforeAudited = ev.Time, true
		p.writeToolAudit(ev, "adk.tool.before", securedFields(sc), map[string]any{"args": ev.ToolArgs}, ev.Time)
		rememberSecuredCall(id, sc)
		return
	}
	if a := agentToolOf(ev.Tool); a != nil {
		kind := agentToolFields(a)
		p.writeToolAudit(ev, "adk.tool.before", kind, nil, ev.Time)
		if id != "" {
			p.agentTools.Store(id, kind)
		}
		return
	}
	if id != "" {
		toolAudits.Store(id, pendingToolAudit{p: p, ev: ev})
	}
}

// toolEnd audits a finished tool call — adk.tool.after for a secured tool
// or an agent run as a tool, and a secured tool's adk.tool.before if it was
// not written as the call started — and records the token usage of the
// model response that asked for it. response is what the model receives;
// ok is false when the call failed.
func (p *weilPlugin) toolEnd(ev *Event, response map[string]any, ok bool) {
	toolAudits.Delete(ev.FunctionCallID)
	sc, secured := takeSecuredCall(ev.FunctionCallID)
	kind, isAgent := p.agentTools.LoadAndDelete(ev.FunctionCallID)
	var usage modelUsage
	var charged bool
	if ok {
		usage, charged = p.usage.claim(ev.FunctionCallID)
	} else {
		p.usage.byCall.Delete(ev.FunctionCallID)
	}
	var after map[string]any
	switch {
	case secured:
		if !sc.beforeAudited {
			p.writeToolAudit(ev, "adk.tool.before", securedFields(sc), map[string]any{"args": ev.ToolArgs}, sc.at)
		}
		if ok {
			// The tool's own return value, as an after-tool callback gets it.
			after = merge(securedFields(sc), map[string]any{"response": returnValue(ev, response)})
		}
	case isAgent && ok:
		after = merge(kind.(map[string]any))
	}
	if after != nil {
		if charged && usage.metadata != nil {
			after["usage_metadata"] = usage.metadata
		}
		p.writeToolAudit(ev, "adk.tool.after", after, nil, time.Now())
	}
	if charged && usage.tokens != nil {
		if who, err := p.resolveIdentity(nil, ev); err == nil {
			recordTokenUsage(ev.Context(), who, usage.tokens)
		}
	}
}

// writeToolAudit submits a tool call's adk.tool.* record, stamped at, as the
// acting agent: the call's common fields, then kind (what the tool is) and
// extra.
func (p *weilPlugin) writeToolAudit(ev *Event, event string, kind, extra map[string]any, at time.Time) {
	who, err := p.resolveIdentity(nil, ev)
	if err != nil {
		p.opts.onError(ev, "tool_audit", err)
		return
	}
	e := *ev
	e.Identity = who
	p.submitAudit(&e, event, merge(toolAuditFields(&e), kind, extra), at)
}

// submitAudit encodes an audit record now and submits it in the background,
// signed by ev.Identity.
func (p *weilPlugin) submitAudit(ev *Event, event string, fields map[string]any, at time.Time) {
	call, err := auditCallAt(ev.Identity, event, fields, at)
	if err != nil {
		p.opts.onError(ev, "audit", err)
		return
	}
	submitAudit(ev.Identity, *call, func(err error) { p.opts.onError(ev, "audit", err) })
}

// toolAudits holds, by function call ID, the tool calls a WithSecureA2A
// plugin did not recognise as it started them, so a secured function the
// plugin cannot see into can write its adk.tool.before record as it starts.
var toolAudits sync.Map // function call ID → pendingToolAudit

type pendingToolAudit struct {
	p  *weilPlugin
	ev *Event
}

// auditToolBefore writes adk.tool.before for the call id if a WithSecureA2A
// plugin registered it, and reports whether it did.
func auditToolBefore(id string, sc securedCall) bool {
	v, ok := toolAudits.LoadAndDelete(id)
	if !ok {
		return false
	}
	pa := v.(pendingToolAudit)
	pa.p.writeToolAudit(pa.ev, "adk.tool.before", securedFields(sc), map[string]any{"args": pa.ev.ToolArgs}, sc.at)
	return true
}

// toolAuditFields are the fields every adk.tool.* record carries.
func toolAuditFields(ev *Event) map[string]any {
	fields := map[string]any{
		"agent": nilIfEmpty(ev.AgentName), "tool": nilIfEmpty(ev.ToolName()), "tool_call_id": nilIfEmpty(ev.FunctionCallID),
		"trace_id": nilIfEmpty(traceIDOf(ev.InvocationID)),
	}
	if ev.Identity != nil {
		fields["wallet"] = ev.Identity.Address()
	}
	if ev.Caller != nil && (ev.Identity == nil || ev.Caller.Address != ev.Identity.Address()) {
		fields["caller_wallet"] = ev.Caller.Address
	}
	return fields
}

// securedFields name a secured tool in its audit records: its scope and
// access level.
func securedFields(sc securedCall) map[string]any {
	return map[string]any{"service": sc.service, "access": sc.access}
}

// agentToolFields name an agent run as a tool in its audit records. Its
// request and reply are not recorded: they may carry user content.
func agentToolFields(a agent.Agent) map[string]any {
	f := map[string]any{"tool_kind": "agent", "sub_agent": nilIfEmpty(a.Name())}
	if id := boundTo(a); id != nil {
		f["sub_agent_wallet"] = id.Address()
	}
	return f
}

// agentToolOf returns the agent t runs, if t is an agent tool (NewAgentTool
// or agenttool.New), else nil.
func agentToolOf(t tool.Tool) agent.Agent {
	if at, ok := t.(*weilAgentTool); ok {
		return at.agent
	}
	v := reflect.ValueOf(t)
	if !v.IsValid() || v.Kind() != reflect.Pointer || v.IsNil() || v.Elem().Kind() != reflect.Struct {
		return nil
	}
	if ty := v.Elem().Type(); ty.Name() != "agentTool" || !strings.HasSuffix(ty.PkgPath(), "/tool/agenttool") {
		return nil
	}
	f := v.Elem().FieldByName("agent")
	if !f.IsValid() || f.Kind() != reflect.Interface {
		return nil
	}
	// The field is unexported; read it through its address.
	a, _ := reflect.NewAt(f.Type(), f.Addr().UnsafePointer()).Elem().Interface().(agent.Agent)
	return a
}

// agentType names an agent's kind as audit records carry it (agent_type):
// "LlmAgent", "SequentialAgent", "RemoteA2aAgent"... ADK keeps the kind in an
// exported field of its internal agent state, read here by reflection.
func agentType(a agent.Agent) string {
	if a == nil {
		return ""
	}
	if v := reflect.ValueOf(a); v.Kind() == reflect.Pointer && v.Elem().Kind() == reflect.Struct {
		if f := v.Elem().FieldByName("AgentType"); f.IsValid() && f.Kind() == reflect.String {
			switch kind := f.String(); kind {
			case "LLMAgent":
				return "LlmAgent"
			case "RemoteAgent":
				return "RemoteA2aAgent"
			case "":
			default:
				return kind
			}
		}
	}
	t := fmt.Sprintf("%T", a)
	if i := strings.LastIndex(t, "."); i >= 0 {
		return t[i+1:]
	}
	return t
}
