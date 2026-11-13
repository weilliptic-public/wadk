package weiladk

import (
	"context"
	"errors"
	"fmt"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"
)

// Hook names an ADK lifecycle point an Action can attach to.
type Hook string

const (
	// HookUserMessage fires when a user message arrives. Payload: UserContent.
	HookUserMessage Hook = "user_message"
	// HookBeforeRun fires before a run starts. Payload: UserContent.
	HookBeforeRun Hook = "before_run"
	// HookAfterRun fires after a run completes. A blocking action here can
	// not veto anything; its errors are only reported.
	HookAfterRun Hook = "after_run"
	// HookEvent fires for every event before it is persisted. Payload:
	// SessionEvent.
	HookEvent Hook = "event"

	// HookBeforeAgent and HookAfterAgent fire around each agent's run.
	HookBeforeAgent Hook = "before_agent"
	HookAfterAgent  Hook = "after_agent"

	// HookBeforeModel fires before an LLM call. Payload: LLMRequest.
	HookBeforeModel Hook = "before_model"
	// HookAfterModel fires after an LLM call. Payload: LLMResponse, Err.
	HookAfterModel Hook = "after_model"
	// HookModelError fires when an LLM call fails. Payload: LLMRequest, Err.
	HookModelError Hook = "model_error"

	// HookBeforeTool fires before a tool runs. Payload: Tool, ToolArgs,
	// FunctionCallID.
	HookBeforeTool Hook = "before_tool"
	// HookAfterTool fires after a tool runs, whether or not it failed.
	// Payload: Tool, ToolArgs, ToolResult, Err, FunctionCallID.
	HookAfterTool Hook = "after_tool"
	// HookToolError fires when a tool fails. Payload: Tool, ToolArgs, Err,
	// FunctionCallID.
	HookToolError Hook = "tool_error"
)

// AllHooks lists every hook, in lifecycle order.
var AllHooks = []Hook{
	HookUserMessage, HookBeforeRun, HookBeforeAgent, HookBeforeModel, HookAfterModel,
	HookModelError, HookBeforeTool, HookAfterTool, HookToolError, HookAfterAgent,
	HookEvent, HookAfterRun,
}

// Event is what an Action sees when its hook fires: who is acting, where in
// the run, and the hook's payload. Payload fields not listed for a Hook are
// zero.
type Event struct {
	Hook Hook
	Time time.Time

	AppName      string
	UserID       string
	SessionID    string
	InvocationID string
	AgentName    string
	Branch       string

	// Identity is the identity resolved for this event; it signs the
	// action's call unless the action overrides it.
	Identity *Identity
	// Caller is the verified wallet that called this agent over A2A (see
	// NewA2AServer), or nil for local runs.
	Caller *Caller

	UserContent    *genai.Content
	LLMRequest     *model.LLMRequest
	LLMResponse    *model.LLMResponse
	Tool           tool.Tool
	ToolArgs       map[string]any
	ToolResult     map[string]any
	FunctionCallID string
	SessionEvent   *session.Event
	Err            error

	// AgentContext is set for agent, model and tool hooks; it gives access
	// to session state, artifacts and actions.
	AgentContext agent.Context
	// InvocationContext is set for run, user-message and event hooks.
	InvocationContext agent.InvocationContext
}

// Context returns the Go context of the hook that produced the event.
func (e *Event) Context() context.Context {
	switch {
	case e.AgentContext != nil:
		return e.AgentContext
	case e.InvocationContext != nil:
		return e.InvocationContext
	default:
		return context.Background()
	}
}

// ToolName returns the tool's name, or "" outside tool hooks.
func (e *Event) ToolName() string {
	if e.Tool == nil {
		return ""
	}
	return e.Tool.Name()
}

func eventFromAgentContext(hook Hook, ctx agent.Context) *Event {
	return &Event{
		Hook:         hook,
		Time:         time.Now(),
		AppName:      ctx.AppName(),
		UserID:       ctx.UserID(),
		SessionID:    ctx.SessionID(),
		InvocationID: ctx.InvocationID(),
		AgentName:    ctx.AgentName(),
		Branch:       ctx.Branch(),
		AgentContext: ctx,
		Caller:       callerOf(ctx),
	}
}

func eventFromInvocationContext(hook Hook, ctx agent.InvocationContext) *Event {
	ev := &Event{
		Hook:              hook,
		Time:              time.Now(),
		InvocationID:      ctx.InvocationID(),
		Branch:            ctx.Branch(),
		UserContent:       ctx.UserContent(),
		InvocationContext: ctx,
		Caller:            callerOf(ctx),
	}
	if s := ctx.Session(); s != nil {
		ev.AppName, ev.UserID, ev.SessionID = s.AppName(), s.UserID(), s.ID()
	}
	if a := ctx.Agent(); a != nil {
		ev.AgentName = a.Name()
	}
	return ev
}

func callerOf(ctx context.Context) *Caller {
	c, _ := CallerFromContext(ctx)
	return c
}

// IdentityResolver picks the identity that acts for an event.
type IdentityResolver func(ev *Event) (*Identity, error)

// Static uses one identity for every agent.
func Static(id *Identity) IdentityResolver {
	return func(*Event) (*Identity, error) {
		if id == nil {
			return nil, errors.New("weiladk: nil identity")
		}
		return id, nil
	}
}

// PerAgent gives each named agent its own identity, falling back to fallback
// (which may be nil to make unmapped agents an error).
func PerAgent(byAgent map[string]*Identity, fallback *Identity) IdentityResolver {
	return func(ev *Event) (*Identity, error) {
		if id, ok := byAgent[ev.AgentName]; ok && id != nil {
			return id, nil
		}
		if fallback != nil {
			return fallback, nil
		}
		return nil, fmt.Errorf("weiladk: no identity for agent %q", ev.AgentName)
	}
}
