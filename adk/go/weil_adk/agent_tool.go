package weiladk

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"sync"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/artifact"
	"google.golang.org/adk/v2/memory"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/agenttool"
	"google.golang.org/adk/v2/tool/toolutils"
	"google.golang.org/genai"
)

// AgentToolConfig configures NewAgentTool.
type AgentToolConfig struct {
	// SkipSummarization ends the calling agent's turn with the sub-agent's
	// reply instead of having the model summarize it.
	SkipSummarization bool
	// OutputSchema is the sub-agent's llmagent.Config.OutputSchema, for an
	// agent not built with NewLLMAgent (which records it): its reply is then
	// returned as the parsed object rather than {"result": text}.
	OutputSchema *genai.Schema
	// Plugins are run in the sub-agent's runner in addition to the calling
	// run's Weil plugins.
	Plugins []*plugin.Plugin
}

// NewAgentTool is agenttool.New with the calling run's Weil plugins, as
// ADK-Python's AgentTool(include_plugins=True) propagates its parent's
// plugins. ADK-Go's agenttool runs the agent on a runner of its own without
// them, so the sub-agent's model calls and tool calls would go unaudited and
// untraced, and its secured tools would not see who called.
//
// The sub-agent runs on a fresh in-memory session seeded with the caller's
// state (ADK's "_adk" keys aside), takes {"request": text} — or its input
// schema's object — and returns {"result": text of its last reply}, or the
// parsed object when it has an output schema. As in Python, state the
// sub-run changes is copied back to the caller, and artifacts go to the
// caller's session. An unbound sub-agent acts as the agent that called it.
func NewAgentTool(a agent.Agent, cfg *AgentToolConfig) tool.Tool {
	if cfg == nil {
		cfg = &AgentToolConfig{}
	}
	stock := agenttool.New(a, &agenttool.Config{SkipSummarization: cfg.SkipSummarization})
	out := cfg.OutputSchema
	if out == nil {
		if s, ok := agentSchemas.Load(a); ok {
			out = s.(llmSchemas).output
		}
	}
	return &weilAgentTool{stock: stock.(declaredTool), agent: a, cfg: *cfg, outputSchema: out}
}

// declaredTool is the stock agent tool's method set.
type declaredTool interface {
	tool.Tool
	Declaration() *genai.FunctionDeclaration
	DisplayResultOnSkipSummarization() bool
}

type weilAgentTool struct {
	stock        declaredTool
	agent        agent.Agent
	cfg          AgentToolConfig
	outputSchema *genai.Schema
}

func (t *weilAgentTool) Name() string                            { return t.stock.Name() }
func (t *weilAgentTool) Description() string                     { return t.stock.Description() }
func (t *weilAgentTool) IsLongRunning() bool                     { return t.stock.IsLongRunning() }
func (t *weilAgentTool) Declaration() *genai.FunctionDeclaration { return t.stock.Declaration() }
func (t *weilAgentTool) DisplayResultOnSkipSummarization() bool {
	return t.stock.DisplayResultOnSkipSummarization()
}
func (t *weilAgentTool) ProcessRequest(_ agent.Context, req *model.LLMRequest) error {
	return toolutils.PackTool(req, t)
}

func (t *weilAgentTool) Run(ctx agent.Context, args any) (map[string]any, error) {
	margs, ok := args.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("agent tool expects map[string]any arguments, got %T", args)
	}
	if t.cfg.SkipSummarization {
		if actions := ctx.Actions(); actions != nil {
			actions.SkipSummarization = true
		}
	}

	var content *genai.Content
	if req, only := margs["request"], len(margs) == 1; only && isRequestDeclaration(t.Declaration()) {
		text, ok := req.(string)
		if !ok {
			text = fmt.Sprint(req)
		}
		content = genai.NewContentFromText(text, genai.RoleUser)
	} else {
		data, err := json.Marshal(margs)
		if err != nil {
			return nil, fmt.Errorf("error serializing tool arguments for agent %s: %w", t.agent.Name(), err)
		}
		content = genai.NewContentFromText(string(data), genai.RoleUser)
	}

	sessions := session.InMemoryService()
	r, err := runner.New(runner.Config{
		AppName:         t.agent.Name(),
		Agent:           t.agent,
		SessionService:  sessions,
		ArtifactService: forwardArtifacts{parent: ctx.Artifacts()},
		MemoryService:   memory.InMemoryService(),
		PluginConfig:    runner.PluginConfig{Plugins: append(runPlugins(ctx.InvocationID()), t.cfg.Plugins...)},
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create runner for sub-agent %s: %w", t.agent.Name(), err)
	}

	state := map[string]any{}
	for k, v := range ctx.State().All() {
		if !strings.HasPrefix(k, "_adk") {
			state[k] = v
		}
	}
	sub, err := sessions.Create(ctx, &session.CreateRequest{AppName: t.agent.Name(), UserID: ctx.UserID(), State: state})
	if err != nil {
		return nil, fmt.Errorf("failed to create session for sub-agent %s: %w", t.agent.Name(), err)
	}

	// The sub-run acts as the caller's identity unless its agent has its own.
	runCtx := context.Context(ctx)
	if id := IdentityOf(ctx); id != nil {
		runCtx = ContextWithIdentity(ctx, id)
	}

	var last *session.Event
	for ev, err := range r.Run(runCtx, sub.Session.UserID(), sub.Session.ID(), content, agent.RunConfig{StreamingMode: agent.StreamingModeSSE}) {
		if err != nil {
			return nil, fmt.Errorf("error during execution of sub-agent %s: %w", t.agent.Name(), err)
		}
		if ev.ErrorCode != "" || ev.ErrorMessage != "" {
			return nil, fmt.Errorf("error from sub-agent %q (code: %q, message: %q)", t.agent.Name(), ev.ErrorCode, ev.ErrorMessage)
		}
		for k, v := range ev.Actions.StateDelta {
			if strings.HasPrefix(k, "_adk") {
				continue
			}
			if err := ctx.State().Set(k, v); err != nil {
				return nil, fmt.Errorf("failed to forward state %q from sub-agent %s: %w", k, t.agent.Name(), err)
			}
		}
		if ev.LLMResponse.Content != nil {
			last = ev
		}
	}
	if last == nil {
		return map[string]any{}, nil
	}
	var texts []string
	for _, p := range last.LLMResponse.Content.Parts {
		if p != nil && p.Text != "" && !p.Thought {
			texts = append(texts, p.Text)
		}
	}
	text := strings.Join(texts, "\n")
	if text == "" {
		return map[string]any{}, nil
	}
	if t.outputSchema != nil {
		var parsed map[string]any
		if err := json.Unmarshal([]byte(text), &parsed); err != nil {
			return nil, fmt.Errorf("output validation failed for sub-agent %s: %w", t.agent.Name(), err)
		}
		return parsed, nil
	}
	return map[string]any{"result": text}, nil
}

// isRequestDeclaration reports whether decl takes agenttool's default
// {"request": string}, i.e. the agent has no input schema.
func isRequestDeclaration(decl *genai.FunctionDeclaration) bool {
	if decl == nil || decl.Parameters == nil {
		return true
	}
	p := decl.Parameters.Properties
	return len(p) == 1 && p["request"] != nil
}

// forwardArtifacts stores the sub-run's artifacts in the caller's session,
// as agenttool does.
type forwardArtifacts struct {
	parent agent.Artifacts
}

func (s forwardArtifacts) Save(ctx context.Context, req *artifact.SaveRequest) (*artifact.SaveResponse, error) {
	if s.parent == nil {
		return nil, errNoParentArtifacts
	}
	saveMu.Lock() // parallel sub-agents share the caller's event delta
	defer saveMu.Unlock()
	return s.parent.Save(ctx, req.FileName, req.Part)
}

func (s forwardArtifacts) Load(ctx context.Context, req *artifact.LoadRequest) (*artifact.LoadResponse, error) {
	if s.parent == nil {
		return nil, errNoParentArtifacts
	}
	if req.Version == 0 {
		return s.parent.Load(ctx, req.FileName)
	}
	return s.parent.LoadVersion(ctx, req.FileName, int(req.Version))
}

func (s forwardArtifacts) List(ctx context.Context, _ *artifact.ListRequest) (*artifact.ListResponse, error) {
	if s.parent == nil {
		return nil, errNoParentArtifacts
	}
	return s.parent.List(ctx)
}

func (forwardArtifacts) Delete(context.Context, *artifact.DeleteRequest) error {
	return errArtifactOpUnsupported
}

func (forwardArtifacts) Versions(context.Context, *artifact.VersionsRequest) (*artifact.VersionsResponse, error) {
	return nil, errArtifactOpUnsupported
}

func (forwardArtifacts) GetArtifactVersion(context.Context, *artifact.GetArtifactVersionRequest) (*artifact.GetArtifactVersionResponse, error) {
	return nil, errArtifactOpUnsupported
}

var (
	saveMu                   sync.Mutex
	errNoParentArtifacts     = fmt.Errorf("weiladk: the calling agent has no artifact service")
	errArtifactOpUnsupported = fmt.Errorf("weiladk: a sub-agent run as a tool can save, load and list the caller's artifacts only")
)

// ── run → Weil plugins ───────────────────────────────────────────────────

// livePlugins maps each running invocation to the Weil plugins in its
// runner, which an agent tool called during it passes on.
var livePlugins sync.Map // invocation ID → *pluginSet

type pluginSet struct {
	mu      sync.Mutex
	plugins []*plugin.Plugin
}

func registerRunPlugin(invocationID string, p *plugin.Plugin) {
	if p == nil || invocationID == "" {
		return
	}
	v, _ := livePlugins.LoadOrStore(invocationID, &pluginSet{})
	s := v.(*pluginSet)
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, q := range s.plugins {
		if q == p {
			return
		}
	}
	s.plugins = append(s.plugins, p)
}

func unregisterRun(invocationID string) { livePlugins.Delete(invocationID) }

func runPlugins(invocationID string) []*plugin.Plugin {
	v, ok := livePlugins.Load(invocationID)
	if !ok {
		return nil
	}
	s := v.(*pluginSet)
	s.mu.Lock()
	defer s.mu.Unlock()
	return append([]*plugin.Plugin(nil), s.plugins...)
}

// ── agents' schemas, as NewLLMAgent records them ─────────────────────────

type llmSchemas struct{ input, output *genai.Schema }

var agentSchemas sync.Map // agent.Agent → llmSchemas
