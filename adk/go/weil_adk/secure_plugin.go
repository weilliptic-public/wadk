package weiladk

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
)

// SecureA2AConfig configures WithSecureA2A.
type SecureA2AConfig struct {
	// Service is the scope the root agent is served under, recorded on its
	// adk.agent.* audit records (Python: setup_secure_a2a(service=...)).
	Service string
	// DisableAudit stops the adk.agent.* and adk.tool.* audit records
	// (Python: audit=False). Tracing stays on, as in Python.
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
//   - adk.tool.before / adk.tool.after for each SecuredUser / SecuredAdmin
//     tool call, with its arguments and the response the model receives (a
//     call that fails — e.g. is refused — records only adk.tool.before, as
//     Python's ADK skips after-callbacks on an exception);
//   - a trace of each run (see WithTracing).
//
// NewExecutor installs it on the served side; add it to a calling agent's
// runner for the calling side. Python's setup_secure_a2a also adds the
// Pensieve tools to the root agent: ADK-Go agents cannot gain tools after
// they are built, so add weiladk.PensieveTools() to the root's Tools.
func WithSecureA2A(cfg SecureA2AConfig) PluginOption {
	return func(o *pluginOptions) {
		if o.tracing == nil {
			t := cfg.Trace
			WithTracing(t)(o)
		}
		if cfg.DisableAudit {
			return
		}
		o.secureAudit = true
		o.actions = append(o.actions, AgentAuditAction(cfg.Service))
	}
}

// toolAudits holds, by function call ID, the tool calls a WithSecureA2A
// plugin is about to run, so a secured tool can write its adk.tool.before
// record as it starts.
var toolAudits sync.Map // function call ID → *Event, with the plugin

type pendingToolAudit struct {
	p  *weilPlugin
	ev *Event
}

// auditToolBefore writes adk.tool.before for the call id if a WithSecureA2A
// plugin registered it, and reports whether it did.
func auditToolBefore(id string, sc securedCall) bool {
	v, ok := toolAudits.Load(id)
	if !ok {
		return false
	}
	pa := v.(pendingToolAudit)
	who, err := pa.p.resolveIdentity(nil, pa.ev)
	if err != nil {
		pa.p.opts.onError(pa.ev, "tool_audit", err)
		return false
	}
	ev := *pa.ev
	ev.Identity = who
	call, err := auditCallAt(who, "adk.tool.before", merge(toolAuditFields(&ev, sc), map[string]any{"args": ev.ToolArgs}), sc.at)
	if err != nil {
		return false
	}
	if _, err := who.Invoke(context.WithoutCancel(ev.Context()), *call); err != nil {
		pa.p.opts.onError(&ev, "tool_audit", err)
	}
	return true
}

func toolAuditFields(ev *Event, sc securedCall) map[string]any {
	fields := map[string]any{
		"agent": ev.AgentName, "tool": ev.ToolName(), "tool_call_id": ev.FunctionCallID,
		"service": sc.service, "access": sc.access,
	}
	if ev.Identity != nil {
		fields["wallet"] = ev.Identity.Address()
	}
	if ev.Caller != nil && (ev.Identity == nil || ev.Caller.Address != ev.Identity.Address()) {
		fields["caller_wallet"] = ev.Caller.Address
	}
	return fields
}

// toolAuditCalls are the adk.tool.before / adk.tool.after records of a
// secured tool call that has finished, or nil for any other call.
// response is what the model receives; ok is false when the call failed.
func toolAuditCalls(ev *Event, response map[string]any, ok bool) []*Call {
	sc, found := takeSecuredCall(ev.FunctionCallID)
	if !found {
		return nil
	}
	fields := toolAuditFields(ev, sc)
	calls := []*Call{}
	if !sc.beforeAudited {
		if c, err := auditCallAt(ev.Identity, "adk.tool.before", merge(fields, map[string]any{"args": ev.ToolArgs}), sc.at); err == nil {
			calls = append(calls, c)
		}
	}
	if ok {
		// The tool's own return value, as Python's after-callback gets it.
		after := merge(fields, map[string]any{"response": returnValue(ev, response)})
		if c, err := auditCallAt(ev.Identity, "adk.tool.after", after, time.Now()); err == nil {
			calls = append(calls, c)
		}
	}
	return calls
}

// agentType names an agent's kind as Python's type(agent).__name__ does.
// ADK keeps the kind in an exported field of its internal agent state, read
// here by reflection.
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
