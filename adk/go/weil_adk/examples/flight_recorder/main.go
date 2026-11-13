// Flight recorder: every step an agent takes is appended, signed by the
// agent's own wallet, to a hash-chained WeilStream topic. A second agent with
// a different wallet then reconstructs and verifies that log from the chain.
//
//	go run ./examples/flight_recorder
//
// What it shows:
//   - a NonBlocking Action that publishes to any applet (here WeilStream's
//     publisher), not just the auditor;
//   - the auditor preset alongside it, fire-and-forget;
//   - applet methods as tools, called as the agent;
//   - two agents with distinct on-chain identities, and tamper detection.
//
// Each agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent      identity   <KEY>
//	analyst    analyst    ANALYST
//	inspector  inspector  INSPECTOR
//
// The impostor (IMPOSTOR) is not an agent, only a wallet that tries to write
// the analyst's log. No grants are needed: each wallet calls public
// WeilStream, token and auditor methods as itself.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"github.com/google/jsonschema-go/jsonschema"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/tool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

var (
	publisher = weiladk.Systemic("publisher")
	consumer  = weiladk.Systemic("consumer")
	streamSt  = weiladk.Systemic("weilstreamstate")
)

func main() {
	ctx := context.Background()
	analyst := devnet.Must(devnet.Agent(ctx, "analyst"))
	inspector := devnet.Must(devnet.Agent(ctx, "inspector"))
	impostor := devnet.Must(devnet.Agent(ctx, "impostor"))
	fmt.Printf("analyst   wallet %s\ninspector wallet %s\nimpostor  wallet %s\n", analyst.Address(), inspector.Address(), impostor.Address())

	topic := fmt.Sprintf("flight-analyst-%d", time.Now().Unix())
	devnet.Must(devnet.SetupCall(ctx, analyst, weiladk.Call{
		Applet: publisher, Method: "create_topic",
		Args: map[string]any{"topic": topic, "description": "ADK flight recorder for the analyst agent", "retention_in_bytes": 10 << 20},
	}))
	fmt.Println("flight log topic:", topic)

	// ── 1. The analyst works; its plugin records every step on chain. ──
	devnet.Section("analyst at work (every step recorded to " + topic + ")")
	analystAgent := devnet.Must(weiladk.NewLLMAgent(analyst, llmagent.Config{
		Name:        "analyst",
		Model:       devnet.Claude(),
		Description: "Answers questions about on-chain token state.",
		Instruction: "You are an analyst with read access to WeilChain applets. Use the tools to gather facts before answering; " +
			"cite the numbers you found. Keep the final report under 150 words.",
		Tools: chainReadTools(),
	}))

	recorder := weiladk.Action{
		Name:  "flight-recorder",
		Hooks: []weiladk.Hook{weiladk.HookUserMessage, weiladk.HookBeforeTool, weiladk.HookAfterTool, weiladk.HookAfterAgent},
		Mode:  weiladk.NonBlocking,
		Build: func(ev *weiladk.Event) (*weiladk.Call, error) {
			entry := map[string]any{
				"hook":       ev.Hook,
				"agent":      ev.AgentName,
				"invocation": ev.InvocationID,
				"at":         ev.Time.UTC().Format(time.RFC3339Nano),
			}
			if ev.UserContent != nil && ev.Hook == weiladk.HookUserMessage {
				entry["prompt"] = ev.UserContent.Parts[0].Text
			}
			if ev.Tool != nil {
				entry["tool"], entry["args"] = ev.ToolName(), ev.ToolArgs
				if ev.ToolResult != nil {
					entry["result_sha256"] = weiladk.HashJSON(ev.ToolResult)
				}
				if ev.Err != nil {
					entry["error"] = ev.Err.Error()
				}
			}
			payload, err := json.Marshal(entry)
			if err != nil {
				return nil, err
			}
			return &weiladk.Call{Applet: publisher, Method: "publish", Args: map[string]any{"topic": topic, "payload": string(payload)}}, nil
		},
	}

	p := devnet.Must(weiladk.NewPlugin(plugin.Config{Name: "flight-recorder"},
		weiladk.WithActions(
			recorder,
			weiladk.AuditAction(weiladk.AuditConfig{Hooks: []weiladk.Hook{weiladk.HookAfterTool, weiladk.HookAfterRun}}),
		),
		// One worker keeps the log in hook order.
		weiladk.WithDispatcher(weiladk.NewDispatcher(weiladk.DispatcherConfig{Workers: 1})),
		weiladk.WithFlushAfterRun(2*time.Minute),
	))
	r := devnet.NewRunner("flight-recorder", analystAgent, runner.PluginConfig{Plugins: []*plugin.Plugin{p}})
	devnet.Must(devnet.Run(ctx, r, "analyst-session",
		fmt.Sprintf("Give me a short status report on the WUSD stablecoin (name, symbol, decimals) and on the token holdings of wallet %s "+
			"in both WUSD and the Ledger. Also tell me how many entries the topic %q holds so far.", analyst.Address(), topic)))
	devnet.Must(0, closePlugin(p))

	// ── 2. Someone else tries to slip an entry into the analyst's log. ──
	devnet.Section("impostor tries to forge a log entry")
	forged := `{"hook":"after_tool","agent":"analyst","tool":"transfer","args":{"to":"impostor","amount":1000000}}`
	// Resent through a busy platform's timeouts (client.DurableRetry): the
	// forged entry is meant to land, so the inspector has something to catch.
	if _, err := impostor.Invoke(ctx, weiladk.Call{Applet: publisher, Method: "publish", Retry: client.DurableRetry,
		Args: map[string]any{"topic": topic, "payload": forged}}); err != nil {
		var appletErr *transaction.AppletError
		switch {
		case errors.As(err, &appletErr) && client.PlatformTimeout(appletErr):
			fmt.Println("forged publish timed out on the platform (outcome unknown):", appletErr.Message)
		case errors.As(err, &appletErr):
			fmt.Println("chain rejected the forged entry:", appletErr.Message)
		default:
			fmt.Println("forged publish failed:", err)
		}
	} else {
		fmt.Println("forged entry accepted by the stream — it is signed by the impostor's wallet, so the inspector should catch it")
	}

	// ── 3. An independent inspector verifies the log from the chain. ──
	devnet.Section("inspector verifies the flight log")
	inspectorAgent := devnet.Must(weiladk.NewLLMAgent(inspector, llmagent.Config{
		Name:  "inspector",
		Model: devnet.Claude(),
		Instruction: "You are an independent auditor. Reconstruct an agent's session from its on-chain flight log and verify it. " +
			"Read every entry, check each entry's payload hash on chain with verify_entry, and check that every entry was published " +
			"by the expected wallet. Report: the sequence of actions the agent took, and a verdict listing any entry that fails a check, citing its seq.",
		Tools: inspectorTools(),
	}))
	ip := devnet.Must(weiladk.NewPlugin(plugin.Config{Name: "inspector"},
		weiladk.WithActions(weiladk.AuditAction(weiladk.AuditConfig{Hooks: []weiladk.Hook{weiladk.HookAfterRun}})),
	))
	ir := devnet.NewRunner("inspector", inspectorAgent, runner.PluginConfig{Plugins: []*plugin.Plugin{ip}})
	devnet.Must(devnet.Run(ctx, ir, "inspector-session",
		fmt.Sprintf("Audit the flight log in topic %q. The only wallet allowed to write it is the analyst's: %s.", topic, analyst.Address())))
	devnet.Must(0, closePlugin(ip))
}

func closePlugin(p *plugin.Plugin) error { return p.Close() }

func chainReadTools() []tool.Tool {
	addr := obj(map[string]*jsonschema.Schema{"addr": str("Wallet address (72 hex chars).")}, "addr")
	return []tool.Tool{
		appletTool("wusd_details", "Returns the WUSD stablecoin's (name, symbol, decimals).", weiladk.Systemic("wusd"), "details", obj(nil)),
		appletTool("wusd_balance", "Returns a wallet's WUSD balance in base units.", weiladk.Systemic("wusd"), "balance_for", addr),
		appletTool("ledger_balances", "Lists a wallet's token balances on the Ledger applet as (token, amount) pairs.", weiladk.Systemic("Ledger"), "balances_for", addr),
		appletTool("stream_topic_info", "Returns metadata of a WeilStream topic, including its next sequence number.", publisher, "topic_info",
			obj(map[string]*jsonschema.Schema{"topic": str("Topic name.")}, "topic")),
	}
}

func inspectorTools() []tool.Tool {
	return []tool.Tool{
		appletTool("read_log", "Reads up to max_count entries of a WeilStream topic starting at from_seq. Each entry has seq, payload, payload_hash, publisher (wallet), block_height and the hash chain links.",
			consumer, "read_batch", obj(map[string]*jsonschema.Schema{
				"topic": str("Topic name."), "from_seq": {Type: "integer"}, "max_count": {Type: "integer"},
			}, "topic", "from_seq", "max_count")),
		appletTool("verify_entry", "Checks on chain that the entry at seq in topic has exactly this payload_hash. Returns true or false.",
			streamSt, "verify", obj(map[string]*jsonschema.Schema{
				"topic": str("Topic name."), "seq": {Type: "integer"}, "payload_hash": str("Hex SHA-256 payload hash."),
			}, "topic", "seq", "payload_hash")),
	}
}

// appletTool exposes an applet method; it signs as whichever agent calls it.
func appletTool(name, desc string, applet weiladk.Applet, method string, schema *jsonschema.Schema) tool.Tool {
	return devnet.Must(weiladk.NewAppletTool(weiladk.AppletToolConfig{
		Name: name, Description: desc, Applet: applet, Method: method, InputSchema: schema,
	}))
}

func obj(props map[string]*jsonschema.Schema, required ...string) *jsonschema.Schema {
	if props == nil {
		props = map[string]*jsonschema.Schema{}
	}
	return &jsonschema.Schema{Type: "object", Properties: props, Required: required}
}

func str(desc string) *jsonschema.Schema {
	return &jsonschema.Schema{Type: "string", Description: desc}
}
