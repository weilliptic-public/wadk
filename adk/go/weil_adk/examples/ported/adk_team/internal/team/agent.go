package team

import (
	"context"
	"os"
	"strings"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/cmd/launcher"
	"google.golang.org/adk/v2/cmd/launcher/full"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
)

// Secured is @weil_ai.secured_user(WEIL_ADK_TEAM_SERVICE).
func Secured[In, Out any](fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	return weiladk.SecuredUser(Service(), fn)
}

// Tool is functiontool.New, exiting on a schema error. description is the
// Python function's docstring, which Python's FunctionTool sends as the
// tool's description.
func Tool[In, Out any](name, description string, fn functiontool.Func[In, Out]) tool.Tool {
	t, err := functiontool.New(functiontool.Config{Name: name, Description: strings.TrimSpace(description)}, fn)
	if err != nil {
		Fatalf("tool %s: %v", name, err)
	}
	return t
}

// SecuredTool is Tool(name, description, Secured(fn)).
func SecuredTool[In, Out any](name, description string, fn functiontool.Func[In, Out]) tool.Tool {
	return Tool(name, description, Secured(fn))
}

// RootAgent is
//
//	@weil_ai.agent(identity=WEIL_ADK_TEAM_AGENT_IDENTITY, verify=False)
//	def create_root_agent(): return Agent(...)
//	weil_root_agent = create_root_agent()
//	weil_root_agent.setup_secure_a2a()
//
// for an LLM agent: the team identity bound to it, and the Pensieve tools
// appended to its tools (setup_secure_a2a's pensieve=True). The rest of
// setup_secure_a2a — audits and traces — is the runner's Plugin.
func RootAgent(ctx context.Context, cfg llmagent.Config) agent.Agent {
	cfg.Tools = append(cfg.Tools, weiladk.PensieveTools()...)
	a, err := weiladk.NewLLMAgent(Identity(ctx), cfg)
	if err != nil {
		Fatalf("agent %s: %v", cfg.Name, err)
	}
	return a
}

// Plugin is the audit and tracing half of setup_secure_a2a() (no service:
// these agents are not served), signed by each agent's identity.
func Plugin() *plugin.Plugin {
	p, err := weiladk.NewPlugin(plugin.Config{},
		weiladk.WithSecureA2A(weiladk.SecureA2AConfig{}),
		weiladk.WithFlushAfterRun(time.Minute),
		weiladk.WithErrorHandler(func(ev *weiladk.Event, action string, err error) {
			Log.Warningf("weil %s failed for %s: %v", action, ev.AgentName, err)
		}))
	if err != nil {
		Fatalf("plugin: %v", err)
	}
	return p
}

// Runner is InMemoryRunner(agent=root, app_name=app) with root's
// setup_secure_a2a hooks installed as a plugin.
type Runner struct {
	*runner.Runner
	App      string
	Sessions session.Service
}

// NewRunner builds the runner.
func NewRunner(app string, root agent.Agent) *Runner {
	return NewRunnerWith(app, root, session.InMemoryService())
}

// NewRunnerWith builds the runner over a given session service.
func NewRunnerWith(app string, root agent.Agent, sessions session.Service) *Runner {
	r, err := runner.New(runner.Config{
		AppName:        app,
		Agent:          root,
		SessionService: sessions,
		PluginConfig:   runner.PluginConfig{Plugins: []*plugin.Plugin{Plugin()}},
	})
	if err != nil {
		Fatalf("runner: %v", err)
	}
	return &Runner{Runner: r, App: app, Sessions: sessions}
}

// NewSession is runner.session_service.create_session(app_name, user_id).
func (r *Runner) NewSession(ctx context.Context, userID string) (string, error) {
	resp, err := r.Sessions.Create(ctx, &session.CreateRequest{AppName: r.App, UserID: userID})
	if err != nil {
		return "", err
	}
	return resp.Session.ID(), nil
}

// Send runs prompt in the session and yields its events.
func (r *Runner) Send(ctx context.Context, userID, sessionID, prompt string, yield func(*session.Event) error) error {
	msg := genai.NewContentFromText(prompt, genai.RoleUser)
	for ev, err := range r.Run(ctx, userID, sessionID, msg, agent.RunConfig{}) {
		if err != nil {
			return err
		}
		if err := yield(ev); err != nil {
			return err
		}
	}
	return nil
}

// CallAgent is the agents' call_agent_async: the text of every event not
// authored by the user, concatenated.
func (r *Runner) CallAgent(ctx context.Context, userID, sessionID, prompt string) (string, error) {
	var out strings.Builder
	err := r.Send(ctx, userID, sessionID, prompt, func(ev *session.Event) error {
		if ev.Content != nil && len(ev.Content.Parts) > 0 && ev.Author != "user" {
			out.WriteString(Text(ev.Content))
		}
		return nil
	})
	return out.String(), err
}

// Text is "".join(part.text or "" for part in content.parts).
func Text(c *genai.Content) string {
	if c == nil {
		return ""
	}
	var b strings.Builder
	for _, p := range c.Parts {
		if p != nil {
			b.WriteString(p.Text)
		}
	}
	return b.String()
}

// Launch serves root with ADK's full launcher (console, web, api, a2a) — the
// Go counterpart of loading a Python agent module with `adk run` / `adk web`
// — with root's setup_secure_a2a hooks as the plugin.
func Launch(ctx context.Context, root agent.Agent) {
	cfg := &launcher.Config{
		AgentLoader:  agent.NewSingleLoader(root),
		PluginConfig: runner.PluginConfig{Plugins: []*plugin.Plugin{Plugin()}},
	}
	l := full.NewLauncher()
	if err := l.Execute(ctx, cfg, os.Args[1:]); err != nil {
		Fatalf("Run failed: %v\n\n%s", err, l.CommandLineSyntax())
	}
}
