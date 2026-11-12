// Quickstart: the whole flow in one file, written the way you already write
// ADK — each step is the ADK call you know, or its weiladk twin.
//
//  1. load the agent's identity   devnet.Agent (weiladk.FromIdentity / FromWalletFile)
//
//  2. attach it to an agent       llmagent.New      → weiladk.NewLLMAgent(id, cfg)
//
//  3. on-chain tools              weiladk.NewAppletTool
//
//  4. hooks with custom logic     plugin.Config callbacks, using weiladk.Invoke / weiladk.Submit
//
//  5. make the plugin             plugin.New(cfg)   → weiladk.NewPlugin(cfg, opts...)
//
//  6. run                         runner.New + Run, unchanged
//
//     go run ./examples/quickstart
//
// The one agent, "assistant", acts under identity quickstart-agent:
//
//	WEIL_IDENTITY_QUICKSTART_AGENT  its Agent Registry identity (weil_...), or
//	WEIL_WALLET_QUICKSTART_AGENT    a wallet.wc file, e.g.
//	                                WEIL_WALLET_QUICKSTART_AGENT=~/.weilliptic/wallet.wc
//	                                (neither: ~/.weilliptic/agents/quickstart-agent.wc,
//	                                created on first use)
//
// It needs no grants: it reads WUSD and publishes to a topic it creates.
//
// Environment: ANTHROPIC_API_KEY (or API_KEY), WEIL_SENTINEL_HOST (default
// https://sentinel.weilliptic.ai; Sentinel's certificate is verified
// except for a local platform).
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"github.com/google/jsonschema-go/jsonschema"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/claudemodel"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

const topic = "quickstart-tool-calls"

func main() {
	ctx := context.Background()

	// ── 1. Load the agent's identity ────────────────────────────────────
	// WEIL_IDENTITY_QUICKSTART_AGENT or WEIL_WALLET_QUICKSTART_AGENT; only a
	// local platform's self-signed certificate is accepted unverified.
	id, err := devnet.Agent(ctx, "quickstart-agent")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("agent wallet:", id.Address())

	// ── 3. An on-chain tool (signs as whichever agent calls it) ─────────
	balanceTool, err := weiladk.NewAppletTool(weiladk.AppletToolConfig{
		Name:        "wusd_balance",
		Description: "Returns a wallet's WUSD balance.",
		Applet:      weiladk.Systemic("wusd"), // "wusd::weil"
		Method:      "balance_for",
		InputSchema: &jsonschema.Schema{
			Type:       "object",
			Properties: map[string]*jsonschema.Schema{"addr": {Type: "string", Description: "Wallet address."}},
			Required:   []string{"addr"},
		},
	})
	if err != nil {
		log.Fatal(err)
	}

	// ── 2. The agent: llmagent.New's twin, plus the wallet ──────────────
	llm, err := claudemodel.NewModel(ctx, "claude-opus-5-5", &claudemodel.Config{APIKey: os.Getenv("API_KEY")})
	if err != nil {
		log.Fatal(err)
	}
	assistant, err := weiladk.NewLLMAgent(id, llmagent.Config{
		Name:        "assistant",
		Model:       llm,
		Instruction: "Answer questions about WeilChain balances using your tools.",
		Tools:       []tool.Tool{balanceTool},
	})
	if err != nil {
		log.Fatal(err)
	}

	// ── 4. Hooks: ordinary ADK callbacks; weiladk.Invoke / Submit act as the agent ─
	ensureTopic(ctx, id)
	hooks := plugin.Config{
		Name: "quickstart",

		// Pre-hook (blocking): ask the chain before any tool runs; an error vetoes the tool.
		BeforeToolCallback: func(ctx agent.Context, t tool.Tool, args map[string]any) (map[string]any, error) {
			res, err := weiladk.Invoke(ctx, weiladk.Call{Applet: weiladk.Systemic("wusd"), Method: "details"})
			if err != nil {
				return nil, fmt.Errorf("token check failed, refusing %s: %w", t.Name(), err)
			}
			var details []any // ["WUSD","WUSD",6]
			if err := res.Decode(&details); err != nil || len(details) == 0 {
				return nil, fmt.Errorf("token not live, refusing %s", t.Name())
			}
			fmt.Printf("  [hook] before %s: token %v is live, allowing\n", t.Name(), details[0])
			return nil, nil // nil, nil = let the tool run
		},

		// Post-hook (non-blocking): publish each finished tool call to WeilStream.
		AfterToolCallback: func(ctx agent.Context, t tool.Tool, args, result map[string]any, toolErr error) (map[string]any, error) {
			payload, _ := json.Marshal(map[string]any{"agent": ctx.AgentName(), "tool": t.Name(), "args": args, "failed": toolErr != nil})
			err := weiladk.Submit(ctx, weiladk.Call{
				Applet: weiladk.Systemic("publisher"), Method: "publish",
				Args: map[string]any{"topic": topic, "payload": string(payload)},
			}, func(res *weiladk.Result, err error) {
				if err == nil {
					fmt.Printf("  [hook] after %s: published to %s at block %d\n", t.Name(), topic, res.Txn.BlockHeight)
				}
			})
			return nil, err // nil, nil = keep the tool's result
		},
	}

	// ── 5. The plugin: plugin.New's twin, with optional declarative extras ─
	p, err := weiladk.NewPlugin(hooks,
		weiladk.WithActions(weiladk.AuditAction(weiladk.AuditConfig{})), // audit trail preset
		weiladk.WithFlushAfterRun(time.Minute),                          // runs end once their records land
	)
	if err != nil {
		log.Fatal(err)
	}
	defer p.Close()

	// ── 6. Run: plain ADK ───────────────────────────────────────────────
	r, err := runner.New(runner.Config{
		AppName:           "quickstart",
		Agent:             assistant,
		SessionService:    session.InMemoryService(),
		PluginConfig:      runner.PluginConfig{Plugins: []*plugin.Plugin{p}},
		AutoCreateSession: true,
	})
	if err != nil {
		log.Fatal(err)
	}

	msg := genai.NewContentFromText("What is the WUSD balance of "+id.Address()+"?", genai.RoleUser)
	for ev, err := range r.Run(ctx, "user-1", "session-1", msg, agent.RunConfig{}) {
		if err != nil {
			log.Fatal(err)
		}
		if ev.IsFinalResponse() && ev.Content != nil {
			for _, part := range ev.Content.Parts {
				if !part.Thought {
					fmt.Println(part.Text)
				}
			}
		}
	}
}

// ensureTopic creates the WeilStream topic the post-hook writes to.
func ensureTopic(ctx context.Context, id *weiladk.Identity) {
	_, err := id.Invoke(ctx, weiladk.Call{
		Applet: weiladk.Systemic("publisher"), Method: "create_topic",
		Args: map[string]any{"topic": topic, "description": "quickstart tool calls", "retention_in_bytes": 1 << 20},
	})
	if err != nil && !strings.Contains(err.Error(), "exists") {
		log.Fatal(err)
	}
}
