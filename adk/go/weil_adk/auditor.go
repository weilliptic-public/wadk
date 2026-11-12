package weiladk

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// DefaultAuditHooks are the hooks AuditAction records when none are given:
// each user message, agent boundary, model response and tool call.
var DefaultAuditHooks = []Hook{
	HookUserMessage, HookBeforeAgent, HookAfterAgent, HookAfterModel,
	HookBeforeTool, HookAfterTool, HookAfterRun,
}

// AuditConfig configures AuditAction. Zero values pick the defaults.
type AuditConfig struct {
	// Name of the action. Default "audit".
	Name string
	// Hooks to record. Default DefaultAuditHooks.
	Hooks []Hook
	// Mode defaults to NonBlocking, so auditing never slows the agent. Use
	// Blocking with FailClosed to refuse to proceed with unaudited steps.
	Mode      Mode
	OnFailure FailurePolicy
	// When, if set, filters the events recorded.
	When func(ev *Event) bool
	// Record builds the log entry for an event. Default AuditRecord, which
	// records hashes rather than raw prompts, arguments and results.
	Record func(ev *Event) (any, error)
	// Applet receiving the entries. Default Auditor ("auditor::weil").
	Applet Applet
	// AcceptOnly counts an entry done once the platform has accepted it,
	// without waiting for the auditor to run it. Faster under load, but an
	// entry the platform accepts and then loses, or the auditor rejects (a
	// free tier used up), is gone without a word. By default each entry
	// waits to run and a lost one is resent; NonBlocking entries wait on a
	// dispatcher worker, not the agent.
	AcceptOnly bool
	// WaitForResult is the default and has no effect.
	//
	// Deprecated: entries wait for the auditor unless AcceptOnly.
	WaitForResult bool
	// FireAndForget has no effect.
	//
	// Deprecated: use AcceptOnly.
	FireAndForget bool
}

// AuditAction records agent activity through the auditor applet's
// `audit(log, org)` method, signed by the acting agent's identity. It is an
// ordinary Action: combine it with others, or build a different audit trail
// on the same primitives.
//
// Entries are not lost to load, transient failures or the platform losing
// a submission: each is signed once, waits for the auditor to run it, and is
// resent with the same transaction ID (client.DurableRetry) until it runs or
// the signed transaction expires.
func AuditAction(cfg AuditConfig) Action {
	if cfg.Name == "" {
		cfg.Name = "audit"
	}
	if len(cfg.Hooks) == 0 {
		cfg.Hooks = DefaultAuditHooks
	}
	if cfg.Record == nil {
		cfg.Record = AuditRecord
	}
	if cfg.Applet.IsZero() {
		cfg.Applet = Auditor
	}

	return Action{
		Name:      cfg.Name,
		Hooks:     cfg.Hooks,
		Mode:      cfg.Mode,
		OnFailure: cfg.OnFailure,
		When:      cfg.When,
		Build: func(ev *Event) (*Call, error) {
			record, err := cfg.Record(ev)
			if err != nil || record == nil {
				return nil, err
			}
			log, err := json.Marshal(record)
			if err != nil {
				return nil, err
			}

			return &Call{
				Applet:        cfg.Applet,
				Method:        "audit",
				Args:          pyArgs(client.AuditArgs(string(log), ev.Identity.Org())),
				FireAndForget: cfg.AcceptOnly,
				Retry:         client.DurableRetry,
			}, nil
		},
	}
}

// AuditRecord is the default audit entry: who acted, where in the run, and
// SHA-256 digests of the payloads (see HashJSON) instead of their contents.
func AuditRecord(ev *Event) (any, error) {
	r := map[string]any{
		"v":          1,
		"hook":       ev.Hook,
		"time":       ev.Time.UTC().Format(time.RFC3339Nano),
		"app":        ev.AppName,
		"user":       ev.UserID,
		"session":    ev.SessionID,
		"invocation": ev.InvocationID,
		"agent":      ev.AgentName,
	}
	if ev.Branch != "" {
		r["branch"] = ev.Branch
	}
	if ev.Identity != nil {
		r["wallet"] = ev.Identity.Address()
	}
	if ev.UserContent != nil {
		r["user_content_sha256"] = HashJSON(ev.UserContent)
	}
	if ev.LLMRequest != nil {
		r["model"] = ev.LLMRequest.Model
		r["request_sha256"] = HashJSON(ev.LLMRequest.Contents)
	}
	if resp := ev.LLMResponse; resp != nil {
		if resp.Content != nil {
			r["response_sha256"] = HashJSON(resp.Content)
		}
		if resp.ModelVersion != "" {
			r["model_version"] = resp.ModelVersion
		}
		if resp.FinishReason != "" {
			r["finish_reason"] = resp.FinishReason
		}
		if u := resp.UsageMetadata; u != nil {
			r["usage"] = map[string]any{
				"prompt_tokens":     u.PromptTokenCount,
				"candidates_tokens": u.CandidatesTokenCount,
				"cached_tokens":     u.CachedContentTokenCount,
				"total_tokens":      u.TotalTokenCount,
			}
		}
	}
	if ev.Tool != nil {
		r["tool"] = ev.Tool.Name()
		r["function_call_id"] = ev.FunctionCallID
		r["args_sha256"] = HashJSON(ev.ToolArgs)
		if ev.ToolResult != nil {
			r["result_sha256"] = HashJSON(ev.ToolResult)
		}
	}
	if e := ev.SessionEvent; e != nil {
		r["event_id"] = e.ID
		r["author"] = e.Author
		r["event_sha256"] = HashJSON(e)
	}
	if ev.Err != nil {
		r["error"] = ev.Err.Error()
	}
	return r, nil
}

// HashJSON returns the hex SHA-256 of v's JSON encoding, or "" if v cannot be
// encoded. Map keys encode sorted, so equal maps hash equally.
func HashJSON(v any) string {
	b, err := json.Marshal(v)
	if err != nil {
		return ""
	}
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}
