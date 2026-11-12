package team

import (
	"context"
	"fmt"
	"strings"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
)

// AgentTool is AgentTool(a) for an agent that called setup_secure_a2a
// itself. ADK-Go's agenttool runs the agent on a runner of its own without
// the outer runner's plugins, so a's audits and traces — which Python
// composes onto the agent as callbacks — would be lost; this runs a the same
// way (fresh in-memory session seeded with the caller's non-"_adk" state,
// {"request": string} in, {"result": text of the last reply} out) on a
// runner with a's own Plugin. As ADK-Python's AgentTool does, the sub-run's
// state changes are copied into the caller's state (except ADK's internal
// "_adk" keys), and a run with no reply text yields {"result": ""}.
func AgentTool(a agent.Agent) tool.Tool {
	type request struct {
		Request string `json:"request"`
	}
	p := Plugin()
	return Tool(a.Name(), a.Description(), func(ctx agent.Context, in request) (map[string]any, error) {
		sessions := session.InMemoryService()
		r, err := runner.New(runner.Config{
			AppName:        a.Name(),
			Agent:          a,
			SessionService: sessions,
			PluginConfig:   runner.PluginConfig{Plugins: []*plugin.Plugin{p}},
		})
		if err != nil {
			return nil, fmt.Errorf("failed to create runner")
		}
		state := map[string]any{}
		for k, v := range ctx.State().All() {
			if !strings.HasPrefix(k, "_adk") {
				state[k] = v
			}
		}
		sub, err := sessions.Create(ctx, &session.CreateRequest{AppName: a.Name(), UserID: ctx.UserID(), State: state})
		if err != nil {
			return nil, fmt.Errorf("failed to create session for sub-agent %s: %w", a.Name(), err)
		}
		var last *session.Event
		for ev, err := range r.Run(ctx, sub.Session.UserID(), sub.Session.ID(),
			genai.NewContentFromText(in.Request, genai.RoleUser), agent.RunConfig{}) {
			if err != nil {
				return nil, fmt.Errorf("error during execution of sub-agent %s: %w", a.Name(), err)
			}
			for k, v := range ev.Actions.StateDelta {
				if strings.HasPrefix(k, "_adk") {
					continue
				}
				if err := ctx.State().Set(k, v); err != nil {
					return nil, fmt.Errorf("failed to forward state %q from sub-agent %s: %w", k, a.Name(), err)
				}
			}
			if ev.Content != nil {
				last = ev
			}
		}
		if last == nil {
			return map[string]any{"result": ""}, nil
		}
		var parts []string
		for _, part := range last.Content.Parts {
			if part != nil && part.Text != "" && !part.Thought {
				parts = append(parts, part.Text)
			}
		}
		return map[string]any{"result": strings.Join(parts, "\n")}, nil
	})
}

// DirectContext is an agent.Context for calling a secured tool function
// outside any agent run, as Python's main scripts do after
// set_current_wallet_addr(agent's wallet): the tool authorizes as id.
// Only what weiladk.SecuredUser reads is implemented; it has no session,
// state or artifacts.
func DirectContext(ctx context.Context, id *weiladk.Identity) agent.Context {
	return &directContext{StrictContextMock: agent.NewStrictContextMock(weiladk.ContextWithIdentity(ctx, id))}
}

type directContext struct {
	agent.StrictContextMock
}

func (*directContext) AgentName() string      { return "" }
func (*directContext) InvocationID() string   { return "" }
func (*directContext) FunctionCallID() string { return "" }
func (*directContext) Agent() agent.Agent     { return nil }
