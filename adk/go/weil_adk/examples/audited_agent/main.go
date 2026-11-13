// An ADK agent with a WeilChain identity: every step is audited on chain,
// tool calls are gated by an applet, and an applet method is exposed as a
// tool — all signed by the agent's own wallet.
//
// Run from the weil_adk module root:
//
//	API_KEY=sk-ant-... WEIL_IDENTITY_AUDITED_AGENT=weil_... go run ./examples/audited_agent "what is my balance?"
//
// The one agent, "assistant", acts under identity audited-agent
// (devnet.AgentOn), which must be given — none is created:
//
//	WEIL_IDENTITY_AUDITED_AGENT  its Agent Registry identity (weil_...), or
//	WEIL_WALLET_AUDITED_AGENT    a wallet.wc file to sign with
//
// It runs against any platform: WEIL_SENTINEL_HOST targets a Sentinel
// (default: the dev platform, sentinel.weilliptic.ai), whose TLS
// certificate is verified unless it is a local platform's self-signed one,
// or WEIL_TLS_INSECURE=1 (see devnet.VerifyTLS).
// GATE_APPLET names an applet whose `authorize(tool, args)` method returns
// {"allowed": bool}; when unset, the gate is skipped. The gate and the audit
// are called as audited-agent, so it needs whatever the gate applet requires.
package main

import (
	"context"
	"errors"
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

func main() {
	ctx := context.Background()
	prompt := strings.Join(os.Args[1:], " ")
	if prompt == "" {
		prompt = "Summarise my recent audit activity."
	}

	// Any platform, production included: Sentinel from WEIL_SENTINEL_HOST
	// (default: the dev platform), TLS verified except for a local platform,
	// and the agent's own identity required — none is ever created.
	id, err := devnet.AgentOn(ctx, "audited-agent", devnet.ClientOptions()...)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("agent wallet: %s", id.Address())

	// Any applet method can be a tool; this one reads billing usage from the
	// auditor applet as the agent.
	usage, err := weiladk.NewAppletTool(weiladk.AppletToolConfig{
		Name:        "get_billing_usage",
		Description: "Returns the number of metered audit events for this wallet (or an org) in a billing month.",
		Applet:      weiladk.Auditor,
		Method:      "get_billing_usage",
		InputSchema: &jsonschema.Schema{
			Type: "object",
			Properties: map[string]*jsonschema.Schema{
				"org":   {Type: "string", Description: "Organization to bill; omit for the caller's own wallet."},
				"epoch": {Type: "string", Description: "Billing month as MMYYYY, e.g. 032026 for March 2026."},
			},
			Required: []string{"epoch"},
		},
	})
	if err != nil {
		log.Fatal(err)
	}

	llm, err := claudemodel.NewModel(ctx, "", &claudemodel.Config{APIKey: os.Getenv("API_KEY")}) // "" = default model
	if err != nil {
		log.Fatal(err)
	}
	ag, err := weiladk.NewLLMAgent(id, llmagent.Config{
		Name:        "assistant",
		Model:       llm,
		Instruction: "You help users understand their WeilChain activity.",
		Tools:       []tool.Tool{usage},
	})
	if err != nil {
		log.Fatal(err)
	}

	actions := []weiladk.Action{
		// Non-blocking: audit entries are queued and never slow the agent.
		weiladk.AuditAction(weiladk.AuditConfig{}),
	}
	if gate := os.Getenv("GATE_APPLET"); gate != "" {
		// Blocking: the tool runs only if the applet says so.
		actions = append(actions, weiladk.Action{
			Name:      "tool-gate",
			Hooks:     []weiladk.Hook{weiladk.HookBeforeTool},
			Mode:      weiladk.Blocking,
			OnFailure: weiladk.FailClosed,
			Build: weiladk.CallTo(weiladk.AppletName(gate), "authorize", func(ev *weiladk.Event) (any, error) {
				return map[string]any{"tool": ev.ToolName(), "args": ev.ToolArgs}, nil
			}),
			OnResult: func(ev *weiladk.Event, res *weiladk.Result) error {
				var verdict struct{ Allowed bool }
				if err := res.Decode(&verdict); err != nil {
					return err
				}
				if !verdict.Allowed {
					return errors.New("not authorized by " + gate)
				}
				return nil
			},
		})
	}

	p, err := weiladk.NewPlugin(plugin.Config{Name: "weil-demo"},
		weiladk.WithActions(actions...),
		weiladk.WithFlushAfterRun(time.Minute),
	)
	if err != nil {
		log.Fatal(err)
	}

	r, err := runner.New(runner.Config{
		AppName:           "weil-demo",
		Agent:             ag,
		SessionService:    session.InMemoryService(),
		PluginConfig:      runner.PluginConfig{Plugins: []*plugin.Plugin{p}},
		AutoCreateSession: true,
	})
	if err != nil {
		log.Fatal(err)
	}

	for ev, err := range r.Run(ctx, "user", "session", genai.NewContentFromText(prompt, genai.RoleUser), agent.RunConfig{}) {
		if err != nil {
			log.Fatal(err)
		}
		if ev.Content != nil && ev.IsFinalResponse() {
			for _, part := range ev.Content.Parts {
				fmt.Print(part.Text)
			}
			fmt.Println()
		}
	}
}
