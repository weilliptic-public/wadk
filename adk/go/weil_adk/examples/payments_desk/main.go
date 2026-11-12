// Payments desk: a coordinator runs a compliance agent, then a treasurer
// agent, each acting under its own wallet. Compliance records vendor
// approvals on chain; the treasurer pays invoices in a Ledger token. Before
// any payment leaves, two Blocking actions consult the chain and can veto it:
//
//   - vendor-allowlist: the payee must be approved in the approvals stream,
//     by the compliance wallet specifically;
//
//   - spend-cap: no single payment may exceed 25% of the treasury's on-chain
//     balance.
//
//     go run ./examples/payments_desk
//
// What it shows: per-agent identities (NewLLMAgent / Bind), Blocking actions that gate
// tool calls on applet answers, chained policy actions on one hook, tools
// that call applets directly, and a NonBlocking audit trail for every agent.
//
// Each agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent                    identity    <KEY>
//	desk (sequential root)   desk        DESK
//	compliance               compliance  COMPLIANCE
//	treasurer                treasurer   TREASURER
//
// The vendors vendor-acme, vendor-globex and vendor-shellco (VENDOR_ACME,
// VENDOR_GLOBEX, VENDOR_SHELLCO) are payees only, not agents. No permission
// grants are needed: compliance publishes to a topic it creates, and the
// treasurer mints and pays CRED from its own Ledger balance.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"github.com/google/jsonschema-go/jsonschema"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/workflowagents/sequentialagent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

const (
	symbol        = "CRED"
	treasuryFloat = 100_000
	capPercent    = 25
)

var (
	ledger    = weiladk.Systemic("Ledger")
	publisher = weiladk.Systemic("publisher")
	consumer  = weiladk.Systemic("consumer")
)

func main() {
	ctx := context.Background()
	desk := devnet.Must(devnet.Agent(ctx, "desk"))
	compliance := devnet.Must(devnet.Agent(ctx, "compliance"))
	treasurer := devnet.Must(devnet.Agent(ctx, "treasurer"))
	vendors := map[string]string{}
	for _, v := range []string{"acme", "globex", "shellco"} {
		vendors[v] = devnet.Must(devnet.Agent(ctx, "vendor-"+v)).Address()
	}
	fmt.Printf("compliance wallet %s\ntreasurer  wallet %s\n", compliance.Address(), treasurer.Address())

	// Setup: an approvals stream owned by compliance, and a funded treasury.
	approvals := fmt.Sprintf("vendor-approvals-%d", time.Now().Unix())
	devnet.Must(devnet.SetupCall(ctx, compliance, weiladk.Call{Applet: publisher, Method: "create_topic",
		Args: map[string]any{"topic": approvals, "description": "Vendors approved by compliance", "retention_in_bytes": 1 << 20}}))
	balance := devnet.Must(treasuryBalance(ctx, treasurer))
	if balance < treasuryFloat {
		devnet.Must(treasurer.Invoke(ctx, weiladk.Call{Applet: ledger, Method: "mint",
			Args: map[string]any{"symbol": symbol, "to_addr": treasurer.Address(), "amount": treasuryFloat - balance}}))
	}
	fmt.Printf("treasury funded: %d %s, approvals stream %s\n", devnet.Must(treasuryBalance(ctx, treasurer)), symbol, approvals)

	complianceAgent := devnet.Must(weiladk.NewLLMAgent(compliance, llmagent.Config{
		Name:        "compliance",
		Model:       devnet.Claude(),
		Description: "Reviews vendor applications and records approvals on chain.",
		Instruction: "You are the compliance officer. Review each vendor application in the request. Approve a vendor with approve_vendor " +
			"only if nothing about it is a red flag; do not approve vendors with red flags. Then summarise your decisions in one line per vendor.",
		Tools: []tool.Tool{approveVendorTool(compliance, approvals)},
	}))
	treasurerAgent := devnet.Must(weiladk.NewLLMAgent(treasurer, llmagent.Config{
		Name:        "treasurer",
		Model:       devnet.Claude(),
		Description: "Pays invoices from the treasury.",
		Instruction: fmt.Sprintf("You are the treasurer. Try to pay every invoice in the request with pay_invoice, one call per invoice, "+
			"exactly as invoiced. Payments are checked by on-chain policy and may be refused; never split, resize or retry a refused invoice, "+
			"and never pay outside pay_invoice. Finish with a table of invoice, vendor, amount and outcome, and the treasury balance "+
			"(treasury_balance) at the end. Amounts are in %s.", symbol),
		Tools: []tool.Tool{payInvoiceTool(treasurer), treasuryBalanceTool(treasurer)},
	}))
	root := devnet.Must(sequentialagent.New(sequentialagent.Config{AgentConfig: agent.Config{
		Name:        "desk",
		Description: "Runs compliance review, then payments.",
		SubAgents:   []agent.Agent{complianceAgent, treasurerAgent},
	}}))
	weiladk.Bind(root, desk) // the coordinator acts under its own wallet

	isPayment := func(ev *weiladk.Event) bool { return ev.ToolName() == "pay_invoice" }
	p := devnet.Must(weiladk.NewPlugin(plugin.Config{Name: "payments-desk"},
		weiladk.WithActions(
			// 1. Payee must be approved, by the compliance wallet.
			weiladk.Action{
				Name: "vendor-allowlist", Hooks: []weiladk.Hook{weiladk.HookBeforeTool}, When: isPayment,
				Mode: weiladk.Blocking, OnFailure: weiladk.FailClosed,
				Build: weiladk.CallTo(consumer, "read_batch", func(*weiladk.Event) (any, error) {
					return map[string]any{"topic": approvals, "from_seq": 0, "max_count": 200}, nil
				}),
				OnResult: func(ev *weiladk.Event, res *weiladk.Result) error {
					var entries []struct {
						Publisher string `json:"publisher"`
						Payload   struct {
							Inline string `json:"Inline"`
						} `json:"payload"`
					}
					if err := res.Decode(&entries); err != nil {
						return err
					}
					payee, _ := ev.ToolArgs["address"].(string)
					for _, e := range entries {
						var approval struct{ Address string }
						if json.Unmarshal([]byte(e.Payload.Inline), &approval) == nil && approval.Address == payee {
							if e.Publisher == compliance.Address() {
								return nil
							}
							return fmt.Errorf("POLICY: approval for %s was published by %s, not by compliance", payee, e.Publisher)
						}
					}
					return fmt.Errorf("POLICY: payee %s has no compliance approval on chain", payee)
				},
			},
			// 2. No payment above capPercent of the on-chain treasury balance.
			weiladk.Action{
				Name: "spend-cap", Hooks: []weiladk.Hook{weiladk.HookBeforeTool}, When: isPayment,
				Mode: weiladk.Blocking, OnFailure: weiladk.FailClosed,
				Build: weiladk.CallTo(ledger, "balance_for", func(ev *weiladk.Event) (any, error) {
					return map[string]any{"addr": ev.Identity.Address(), "symbol": symbol}, nil
				}),
				OnResult: func(ev *weiladk.Event, res *weiladk.Result) error {
					var balance uint64
					if err := res.Decode(&balance); err != nil {
						return err
					}
					amount, _ := ev.ToolArgs["amount"].(float64)
					if limit := balance * capPercent / 100; uint64(amount) > limit {
						return fmt.Errorf("POLICY: %d %s exceeds the single-payment cap of %d (%d%% of treasury balance %d); needs board approval",
							uint64(amount), symbol, limit, capPercent, balance)
					}
					return nil
				},
			},
			// Every agent's activity, signed by its own wallet.
			weiladk.AuditAction(weiladk.AuditConfig{}),
		),
		weiladk.WithFlushAfterRun(2*time.Minute),
	))
	defer p.Close()

	r := devnet.NewRunner("payments-desk", root, runner.PluginConfig{Plugins: []*plugin.Plugin{p}})
	devnet.Must(devnet.Run(ctx, r, "desk-session", fmt.Sprintf(`Vendor applications:
- Acme Supplies (address %s): office supplies wholesaler, 14 years trading, audited accounts, references checked.
- Globex Corp (address %s): cloud hosting provider, public company, existing contract renewed annually.
- ShellCo Ltd (address %s): incorporated 2 days ago in an undisclosed jurisdiction, no references, insists on prepayment in full.

Invoices to pay:
- INV-1001  Acme Supplies   12000
- INV-1002  Globex Corp     40000
- INV-1003  ShellCo Ltd      5000
- INV-1004  Globex Corp      9500`, vendors["acme"], vendors["globex"], vendors["shellco"])))

	devnet.Section("on-chain state after the run")
	fmt.Printf("treasury %s balance: %d\n", symbol, devnet.Must(treasuryBalance(ctx, treasurer)))
	for _, v := range []string{"acme", "globex", "shellco"} {
		got := devnet.Must(treasurer.Invoke(ctx, weiladk.Call{Applet: ledger, Method: "balance_for",
			Args: map[string]any{"addr": vendors[v], "symbol": symbol}}))
		var n uint64
		_ = got.Decode(&n)
		fmt.Printf("vendor %-8s received so far: %d\n", v, n)
	}
}

func treasuryBalance(ctx context.Context, treasurer *weiladk.Identity) (uint64, error) {
	res, err := treasurer.Invoke(ctx, weiladk.Call{Applet: ledger, Method: "balance_for",
		Args: map[string]any{"addr": treasurer.Address(), "symbol": symbol}})
	if err != nil {
		return 0, err
	}
	var n uint64
	return n, res.Decode(&n)
}

type approveArgs struct {
	Vendor  string `json:"vendor" jsonschema:"Vendor name."`
	Address string `json:"address" jsonschema:"Vendor wallet address."`
	Reason  string `json:"reason" jsonschema:"Why the vendor is approved."`
}

// approveVendorTool records an approval in the approvals stream, signed by
// the compliance wallet.
func approveVendorTool(compliance *weiladk.Identity, topic string) tool.Tool {
	return devnet.Must(functiontool.New(functiontool.Config{
		Name: "approve_vendor", Description: "Records on chain that a vendor is approved to receive payments.",
	}, func(ctx agent.Context, a approveArgs) (map[string]any, error) {
		payload, _ := json.Marshal(map[string]any{"vendor": a.Vendor, "address": a.Address, "reason": a.Reason})
		res, err := compliance.Invoke(ctx, weiladk.Call{Applet: publisher, Method: "publish", Retry: client.DurableRetry, // an appended record: resent on the platform timeout
			Args: map[string]any{"topic": topic, "payload": string(payload)}})
		if err != nil {
			return nil, err
		}
		var receipt map[string]any
		_ = res.Decode(&receipt)
		return map[string]any{"approved": a.Vendor, "seq": receipt["seq"], "block_height": receipt["block_height"]}, nil
	}))
}

type payArgs struct {
	Invoice string  `json:"invoice" jsonschema:"Invoice number."`
	Vendor  string  `json:"vendor" jsonschema:"Vendor name."`
	Address string  `json:"address" jsonschema:"Vendor wallet address."`
	Amount  float64 `json:"amount" jsonschema:"Amount to pay, in whole tokens."`
}

// payInvoiceTool transfers treasury tokens to a vendor through the Ledger
// applet, signed by the treasurer wallet.
func payInvoiceTool(treasurer *weiladk.Identity) tool.Tool {
	return devnet.Must(functiontool.New(functiontool.Config{
		Name: "pay_invoice", Description: "Pays an invoice from the treasury to the vendor's wallet.",
	}, func(ctx agent.Context, a payArgs) (map[string]any, error) {
		if a.Amount <= 0 {
			return nil, errors.New("amount must be positive")
		}
		res, err := treasurer.Invoke(ctx, weiladk.Call{Applet: ledger, Method: "transfer", Args: map[string]any{
			"symbol": symbol, "from_addr": treasurer.Address(), "to_addr": a.Address, "amount": uint64(a.Amount),
		}})
		if err != nil {
			return nil, err
		}
		return map[string]any{"paid": a.Invoice, "amount": a.Amount, "block_height": res.Txn.BlockHeight}, nil
	}))
}

func treasuryBalanceTool(treasurer *weiladk.Identity) tool.Tool {
	return devnet.Must(weiladk.NewAppletTool(weiladk.AppletToolConfig{
		Name: "treasury_balance", Description: "Returns the treasury's balance of the " + symbol + " token.",
		Applet: ledger, Method: "balance_for",
		InputSchema: &jsonschema.Schema{Type: "object", Properties: map[string]*jsonschema.Schema{
			"addr":   {Type: "string", Description: "Treasury wallet address: " + treasurer.Address()},
			"symbol": {Type: "string", Description: "Token symbol: " + symbol},
		}, Required: []string{"addr", "symbol"}},
	}))
}
