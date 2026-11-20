package client

import (
	"context"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// AgentRegistryApplet is the service name of the systemic agent registry
// applet, which keeps agents' token usage.
const AgentRegistryApplet = "agent_registry::weil"

// TokenUsage is agent_registry's TokenUsage: the tokens one model call read
// and wrote.
type TokenUsage struct {
	InputTokens  int64 `json:"input_tokens"`
	OutputTokens int64 `json:"output_tokens"`
}

// RecordAgentTokenUsage records one model call's token usage on
// agent_registry::weil, resolved with this wallet's address so the call
// lands on the wallet's home pod. timestamp is the UTC ISO-8601 time the
// model response arrived (e.g. "2026-11-15T10:02:11.123Z"). The call is
// submitted without waiting for it to run, its arguments public. It returns
// nil without submitting when the wallet is linked to no org.
func (w *WeilClient) RecordAgentTokenUsage(ctx context.Context, model string, usage TokenUsage, timestamp string) (*transaction.TransactionResult, error) {
	org := w.Org()
	if org == nil || org.Name == "" {
		return nil, nil
	}
	args := struct {
		Org       string     `json:"org"`
		Subgroup  *string    `json:"subgroup"`
		Model     string     `json:"model"`
		Usage     TokenUsage `json:"usage"`
		Timestamp string     `json:"timestamp"`
	}{Org: org.Name, Model: model, Usage: usage, Timestamp: timestamp}
	if org.Subgroup != nil && *org.Subgroup != "" {
		args.Subgroup = org.Subgroup
	}
	argsBytes, err := PythonDumpsValue(args, false)
	if err != nil {
		return nil, err
	}
	return w.ExecuteByName(ctx, AgentRegistryApplet, "record_agent_token_usage", string(argsBytes), ExecuteOptions{NonBlocking: true})
}
