// shop-assistant is one LLM agent with its own WeilChain wallet. It shows:
//
//   - loading the wallet and attaching it to the agent;
//   - four kinds of tool: a plain Go tool, an applet method as a tool, a tool
//     whose own code calls an applet, and the tools of a secured MCP server
//     (orders-mcp, which must be running on 127.0.0.1:9000);
//   - pre/post hooks: a guard before each tool call, a log after each one,
//     an action on each user message, and an on-chain audit of every step.
//
// Every applet call is signed by the shop-assistant wallet.
//
// The one agent, shop_assistant, acts under identity shop-assistant
// (devnet.Agent):
//
//	WEIL_IDENTITY_SHOP_ASSISTANT  its Agent Registry identity (weil_...), or
//	WEIL_WALLET_SHOP_ASSISTANT    a wallet.wc file
//	                              (neither: ~/.weilliptic/agents/shop-assistant.wc,
//	                              created on first use)
//
// orders-mcp serves lookup_order to shop-assistant with Execution (or
// Management) in engg.weil, and refund_order only with Management there.
// Sentinel is WEIL_SENTINEL_HOST (default https://sentinel.weilliptic.ai;
// set WEIL_SENTINEL_HOST=https://sentinel-local.weilliptic.ai for the local
// platform); its TLS certificate is verified except for a local platform.
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
	"github.com/modelcontextprotocol/go-sdk/mcp"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/adk/v2/tool/mcptoolset"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/claudemodel"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

const (
	ordersMCPURL = "http://127.0.0.1:9000/mcp"
	claudeModel  = "claude-opus-5-5"
)

func main() {
	ctx := context.Background()

	// ── 1. Identity: load the agent's wallet ──────────────────────────────
	// WEIL_IDENTITY_SHOP_ASSISTANT, WEIL_WALLET_SHOP_ASSISTANT, else
	// ~/.weilliptic/agents/shop-assistant.wc.
	shopID, err := devnet.Agent(ctx, "shop-assistant")
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("shop-assistant wallet: %s", shopID.Address())

	// The topics this program publishes to on publisher::weil. A topic has
	// to exist before anything is published to it.
	for topic, description := range map[string]string{
		"support-notes": "Notes for the support team",
		"tool-log":      "One entry per tool call of shop_assistant",
		"questions":     "One entry per customer question",
	} {
		if err := createTopic(ctx, shopID, topic, description); err != nil {
			log.Fatalf("create topic %s: %v", topic, err)
		}
	}

	llm, err := claudemodel.NewModel(ctx, claudeModel, &claudemodel.Config{APIKey: os.Getenv("API_KEY")})
	if err != nil {
		log.Fatalf("create model: %v", err)
	}

	// ── 2. Tools ──────────────────────────────────────────────────────────

	// 2a. A plain Go tool: no chain involved.
	type QuoteInput struct {
		UnitPrice float64 `json:"unit_price" jsonschema:"Price of one item, in rupees."`
		Quantity  int     `json:"quantity" jsonschema:"How many items."`
	}
	type QuoteOutput struct {
		Subtotal float64 `json:"subtotal"`
		GST      float64 `json:"gst"`
		Total    float64 `json:"total"`
	}
	quotePrice, err := functiontool.New(
		functiontool.Config{Name: "quote_price", Description: "Computes the total for an order, with 18% GST."},
		func(ctx agent.Context, in QuoteInput) (QuoteOutput, error) {
			subtotal := in.UnitPrice * float64(in.Quantity)
			gst := subtotal * 0.18
			return QuoteOutput{Subtotal: subtotal, GST: gst, Total: subtotal + gst}, nil
		})
	if err != nil {
		log.Fatalf("create quote_price: %v", err)
	}

	// 2b. An applet method as a tool: the model's arguments go straight to
	// websearch::weil's "search" method, signed by the agent's wallet.
	webSearch, err := weiladk.NewAppletTool(weiladk.AppletToolConfig{
		Name:        "web_search",
		Description: "Searches the web for product information.",
		Applet:      weiladk.Systemic("websearch"), // websearch::weil
		Method:      "search",
		InputSchema: &jsonschema.Schema{
			Type: "object",
			Properties: map[string]*jsonschema.Schema{
				"search_query": {Type: "string", Description: "What to search for."},
			},
			Required: []string{"search_query"},
		},
	})
	if err != nil {
		log.Fatalf("create web_search: %v", err)
	}

	// 2c. A tool whose own code calls an applet: weiladk.Invoke signs the
	// call with the wallet of the agent running the tool.
	type NotifyInput struct {
		Message string `json:"message" jsonschema:"A short note for the support team."`
	}
	type NotifyOutput struct {
		Topic string `json:"topic"`
		Sent  bool   `json:"sent"`
	}
	notifyTeam, err := functiontool.New(
		functiontool.Config{Name: "notify_team", Description: "Sends a note to the support team's 'support-notes' topic."},
		func(ctx agent.Context, in NotifyInput) (NotifyOutput, error) {
			_, err := weiladk.Invoke(ctx, weiladk.Call{
				Applet: weiladk.Systemic("publisher"), // publisher::weil
				Method: "publish",
				Args:   map[string]any{"topic": "support-notes", "payload": in.Message},
				Retry:  client.DurableRetry, // an appended note: resent on the platform timeout
			})
			if err != nil {
				return NotifyOutput{}, err
			}
			return NotifyOutput{Topic: "support-notes", Sent: true}, nil
		})
	if err != nil {
		log.Fatalf("create notify_team: %v", err)
	}

	// 2d. MCP tools: lookup_order and refund_order from orders-mcp. Every
	// request is signed by the agent's wallet (shopID.HTTPClient()), and the
	// server checks that wallet's permissions in engg.weil. The server returns
	// an audit entry with each result; weiladk.AuditMCPResults submits it to
	// auditor::weil, signed by shopID.
	ordersClient := mcp.NewClient(&mcp.Implementation{Name: "shop-assistant", Version: "1.0.0"}, nil)
	ordersClient.AddSendingMiddleware(weiladk.AuditMCPResults(shopID))
	ordersTools, err := mcptoolset.New(mcptoolset.Config{
		Client: ordersClient,
		Transport: &mcp.StreamableClientTransport{
			Endpoint:   ordersMCPURL,
			HTTPClient: shopID.HTTPClient(),
		},
	})
	if err != nil {
		log.Fatalf("create orders MCP toolset: %v", err)
	}

	// ── 3. The agent, with its wallet attached ─────────────────────────────
	shopAssistant, err := weiladk.NewLLMAgent(shopID, llmagent.Config{
		Name:        "shop_assistant",
		Description: "Handles customer questions about orders and products.",
		Instruction: "You are a shop's support assistant. Look orders up with lookup_order. " +
			"Refund an order with refund_order only if it was delivered damaged. " +
			"Tell the team about every refund with notify_team. " +
			"Use quote_price for prices and web_search for product questions.",
		Model:    llm,
		Tools:    []tool.Tool{quotePrice, webSearch, notifyTeam},
		Toolsets: []tool.Toolset{ordersTools},
	})
	if err != nil {
		log.Fatalf("create agent: %v", err)
	}

	// ── 4. Hooks ──────────────────────────────────────────────────────────

	// 4a. PRE hook (before each tool call): refuse long notes. Returning an
	// error vetoes the call, and the model sees the error as the tool's result.
	// Returning nil, nil lets the tool run.
	guardNotes := func(ctx agent.Context, t tool.Tool, args map[string]any) (map[string]any, error) {
		if t.Name() != "notify_team" {
			return nil, nil
		}
		message, _ := args["message"].(string)
		if len(message) > 280 {
			return nil, fmt.Errorf("notify_team refused: the note is %d characters, the limit is 280", len(message))
		}
		return nil, nil
	}

	// 4b. POST hook (after each tool call): publish which tool ran, with
	// hashes of its arguments and result, to the 'tool-log' topic.
	// weiladk.Submit queues the call and returns at once, so the agent does
	// not wait for it.
	logToolCall := func(ctx agent.Context, t tool.Tool, args, result map[string]any, toolErr error) (map[string]any, error) {
		entry, err := json.Marshal(map[string]any{
			"tool":          t.Name(),
			"args_sha256":   weiladk.HashJSON(args),
			"result_sha256": weiladk.HashJSON(result),
			"failed":        toolErr != nil,
		})
		if err != nil {
			return nil, err
		}
		err = weiladk.Submit(ctx, weiladk.Call{
			Applet: weiladk.Systemic("publisher"),
			Method: "publish",
			Args:   map[string]any{"topic": "tool-log", "payload": string(entry)},
		}, nil)
		return nil, err // nil result: keep the tool's own result
	}

	// 4c. A declarative action: on each user message, publish the message's
	// length to the 'questions' topic, without blocking the run.
	recordQuestion := weiladk.Action{
		Name:      "record_question",
		Hooks:     []weiladk.Hook{weiladk.HookUserMessage},
		Mode:      weiladk.NonBlocking,
		OnFailure: weiladk.FailOpen,
		Build: weiladk.CallTo(weiladk.Systemic("publisher"), "publish",
			func(ev *weiladk.Event) (any, error) {
				var text strings.Builder
				for _, part := range ev.UserContent.Parts {
					text.WriteString(part.Text)
				}
				payload := fmt.Sprintf(`{"agent":%q,"question_chars":%d}`, ev.AgentName, text.Len())
				return map[string]any{"topic": "questions", "payload": payload}, nil
			}),
	}

	hooks, err := weiladk.NewPlugin(
		plugin.Config{
			Name:               "shop_hooks",
			BeforeToolCallback: guardNotes,
			AfterToolCallback:  logToolCall,
		},
		// 4d. Built-in: audit every step (user message, model call, tool
		// call, ...) on auditor::weil, signed by the agent's wallet.
		weiladk.WithActions(recordQuestion, weiladk.AuditAction(weiladk.AuditConfig{})),
		weiladk.WithErrorHandler(func(ev *weiladk.Event, action string, err error) {
			log.Printf("hook %s failed on %s: %v", action, ev.Hook, err)
		}),
		weiladk.WithFlushAfterRun(time.Minute), // a run ends only once its queued calls have landed
	)
	if err != nil {
		log.Fatalf("create plugin: %v", err)
	}

	// ── 5. Run ────────────────────────────────────────────────────────────
	r, err := runner.New(runner.Config{
		AppName:           "shop",
		Agent:             shopAssistant,
		SessionService:    session.InMemoryService(),
		AutoCreateSession: true,
		PluginConfig:      runner.PluginConfig{Plugins: []*plugin.Plugin{hooks}},
	})
	if err != nil {
		log.Fatalf("create runner: %v", err)
	}

	question := genai.NewContentFromText(
		"Order ORD-1001 arrived damaged. Please check it and sort it out.", genai.RoleUser)

	for event, err := range r.Run(ctx, "customer-42", "session-1", question, agent.RunConfig{}) {
		if err != nil {
			log.Fatalf("run: %v", err)
		}
		if event.ErrorMessage != "" {
			fmt.Printf("%s: error: %s\n", event.Author, event.ErrorMessage)
		}
		if event.Content == nil {
			continue
		}
		for _, part := range event.Content.Parts {
			switch {
			case part.FunctionCall != nil:
				fmt.Printf("→ %s(%v)\n", part.FunctionCall.Name, part.FunctionCall.Args)
			case part.FunctionResponse != nil:
				fmt.Printf("← %s: %v\n", part.FunctionResponse.Name, part.FunctionResponse.Response)
			case part.Text != "" && !event.Partial:
				fmt.Printf("%s: %s\n", event.Author, part.Text)
			}
		}
	}
}

// createTopic creates topic on publisher::weil, signed by id. A topic that
// already exists is fine; devnet.SetupCall also rides out a busy platform.
func createTopic(ctx context.Context, id *weiladk.Identity, topic, description string) error {
	_, err := devnet.SetupCall(ctx, id, weiladk.Call{
		Applet: weiladk.Systemic("publisher"),
		Method: "create_topic",
		Args: map[string]any{
			"topic":              topic,
			"description":        description,
			"retention_in_bytes": 10 << 20, // keep the latest 10 MiB
		},
	})
	return err
}
