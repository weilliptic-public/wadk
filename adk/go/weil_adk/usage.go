package weiladk

import (
	"bytes"
	"context"
	"encoding/json"
	"sync"
	"sync/atomic"
	"time"

	"google.golang.org/adk/v2/model"
	"google.golang.org/genai"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// modelUsage is one model response's token usage: metadata is its usage
// metadata as audit records carry it (usage_metadata), tokens what is
// recorded on agent_registry, or nil when the response used no tokens.
type modelUsage struct {
	metadata json.RawMessage
	tokens   *tokenUsage
}

// tokenUsage is one agent_registry record: the model, its token counts and
// when the response arrived.
type tokenUsage struct {
	model string
	usage client.TokenUsage
	at    string
}

// usageClaim is the usage of a response that asked for tool calls, shared
// by those calls: the first call recorded claims it, so the response is
// charged once however many tools it calls.
type usageClaim struct {
	modelUsage
	claimed atomic.Bool
}

// usageMetadata encodes a response's usage metadata as the audit records
// carry it: the GenAI field names in snake_case, in their declared order,
// fields with no value left out.
func usageMetadata(u *genai.GenerateContentResponseUsageMetadata) json.RawMessage {
	if u == nil {
		return nil
	}
	var b bytes.Buffer
	b.WriteByte('{')
	field := func(name string, v any) {
		if b.Len() > 1 {
			b.WriteByte(',')
		}
		k, _ := json.Marshal(name)
		val, _ := json.Marshal(v)
		b.Write(k)
		b.WriteByte(':')
		b.Write(val)
	}
	count := func(name string, n int32) {
		if n != 0 {
			field(name, n)
		}
	}
	details := func(name string, list []*genai.ModalityTokenCount) {
		if list == nil {
			return
		}
		out := make([]json.RawMessage, 0, len(list))
		for _, d := range list {
			if d == nil {
				continue
			}
			var e bytes.Buffer
			e.WriteByte('{')
			if d.Modality != "" {
				m, _ := json.Marshal(d.Modality)
				e.WriteString(`"modality":`)
				e.Write(m)
			}
			if d.TokenCount != 0 {
				if e.Len() > 1 {
					e.WriteByte(',')
				}
				n, _ := json.Marshal(d.TokenCount)
				e.WriteString(`"token_count":`)
				e.Write(n)
			}
			e.WriteByte('}')
			out = append(out, e.Bytes())
		}
		field(name, out)
	}
	details("cache_tokens_details", u.CacheTokensDetails)
	count("cached_content_token_count", u.CachedContentTokenCount)
	count("candidates_token_count", u.CandidatesTokenCount)
	details("candidates_tokens_details", u.CandidatesTokensDetails)
	count("prompt_token_count", u.PromptTokenCount)
	details("prompt_tokens_details", u.PromptTokensDetails)
	count("thoughts_token_count", u.ThoughtsTokenCount)
	count("tool_use_prompt_token_count", u.ToolUsePromptTokenCount)
	details("tool_use_prompt_tokens_details", u.ToolUsePromptTokensDetails)
	count("total_token_count", u.TotalTokenCount)
	if u.TrafficType != "" {
		field("traffic_type", u.TrafficType)
	}
	b.WriteByte('}')
	return b.Bytes()
}

// usageOf is resp's usage, or false when it reports none. Its token counts
// are agent_registry's: input is the prompt plus the tool-use prompt, output
// the candidates plus the thoughts; the model is the response's model
// version, or "unknown".
func usageOf(resp *model.LLMResponse, at time.Time) (modelUsage, bool) {
	if resp == nil || resp.UsageMetadata == nil {
		return modelUsage{}, false
	}
	u := resp.UsageMetadata
	mu := modelUsage{metadata: usageMetadata(u)}
	in := int64(u.PromptTokenCount) + int64(u.ToolUsePromptTokenCount)
	out := int64(u.CandidatesTokenCount) + int64(u.ThoughtsTokenCount)
	if in != 0 || out != 0 {
		name := resp.ModelVersion
		if name == "" {
			name = "unknown"
		}
		mu.tokens = &tokenUsage{model: name, usage: client.TokenUsage{InputTokens: in, OutputTokens: out}, at: auditTime(at)}
	}
	return mu, true
}

// auditTime formats t as the audit records and agent_registry stamp times:
// UTC ISO-8601 with milliseconds.
func auditTime(t time.Time) string {
	return t.UTC().Format("2006-01-02T15:04:05.000Z")
}

// usageLedger holds the usage of a plugin's model responses until it is
// recorded: by function call ID for responses that asked for tool calls
// (charged to the first of those calls), else by run and agent for the
// agent's adk.agent.after.
type usageLedger struct {
	byCall sync.Map // function call ID → *usageClaim
	mu     sync.Mutex
	byRun  map[agentRunKey][]modelUsage
}

type agentRunKey struct{ invocation, agent string }

// open starts collecting the final-answer usage of an agent's run.
func (l *usageLedger) open(k agentRunKey) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.byRun == nil {
		l.byRun = map[agentRunKey][]modelUsage{}
	}
	l.byRun[k] = []modelUsage{}
}

// take ends an agent's run and returns its final-answer usage.
func (l *usageLedger) take(k agentRunKey) []modelUsage {
	l.mu.Lock()
	defer l.mu.Unlock()
	out := l.byRun[k]
	delete(l.byRun, k)
	return out
}

// drop forgets the run invocation's agents whose end never came (a
// before-agent callback that skipped the agent).
func (l *usageLedger) drop(invocation string) {
	l.mu.Lock()
	defer l.mu.Unlock()
	for k := range l.byRun {
		if k.invocation == invocation {
			delete(l.byRun, k)
		}
	}
}

// note files a model response's usage: under each function call it asks
// for, or else under the agent's run, if that run is open.
func (l *usageLedger) note(k agentRunKey, resp *model.LLMResponse, at time.Time) {
	mu, ok := usageOf(resp, at)
	if !ok {
		return
	}
	var ids []string
	if resp.Content != nil {
		for _, p := range resp.Content.Parts {
			if p != nil && p.FunctionCall != nil {
				ids = append(ids, p.FunctionCall.ID)
			}
		}
	}
	if len(ids) == 0 {
		l.mu.Lock()
		defer l.mu.Unlock()
		if list, open := l.byRun[k]; open {
			l.byRun[k] = append(list, mu)
		}
		return
	}
	claim := &usageClaim{modelUsage: mu}
	for _, id := range ids {
		if id != "" {
			l.byCall.Store(id, claim)
		}
	}
}

// claim returns the usage of the response that asked for the call id, or
// false when there is none or another call of that response claimed it.
func (l *usageLedger) claim(id string) (modelUsage, bool) {
	if id == "" {
		return modelUsage{}, false
	}
	v, ok := l.byCall.LoadAndDelete(id)
	if !ok {
		return modelUsage{}, false
	}
	c := v.(*usageClaim)
	if !c.claimed.CompareAndSwap(false, true) {
		return modelUsage{}, false
	}
	return c.modelUsage, true
}

// recordTokenUsage records u on agent_registry as id. Failures are ignored:
// usage tracking never breaks a run.
func recordTokenUsage(ctx context.Context, id *Identity, u *tokenUsage) {
	if id == nil || u == nil {
		return
	}
	_, _ = id.client.RecordAgentTokenUsage(context.WithoutCancel(ctx), u.model, u.usage, u.at)
}
