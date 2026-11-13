// secured_audit_starter is the Go port of the Python SDK's
// mcp_secured_audit_starter.py: an MCP server with two tools secured by
// WeilChain wallet permissions and audited, and a client agent that signs
// its requests with its wallet, lets an OpenAI model pick a tool, and
// submits the audit entry each tool result carries from its own wallet.
//
//	go run ./examples/secured_audit_starter server
//	go run ./examples/secured_audit_starter client
//
// Server: http://127.0.0.1:8001/mcp. add_numbers and shout serve wallets
// holding Execution or Management in engg.weil; Middleware verifies every
// request's signed X-Wallet-* headers.
//
// The client agent acts under its own identity, secured-audit-starter-client,
// which needs Execution or Management in engg.weil for both tools.
// Client environment (Python hard-codes these; they are never in code here):
//
//	WEIL_IDENTITY_SECURED_AUDIT_STARTER_CLIENT  the client agent's Agent Registry identity, or
//	WEIL_WALLET_SECURED_AUDIT_STARTER_CLIENT    a wallet.wc file instead
//	WEIL_SENTINEL_HOST (or SENTINEL_HOST)       Sentinel base URL for client and server (default: the dev platform)
//	OPENAI_API_KEY                              OpenAI key; OPENAI_BASE_URL for another endpoint
//	OPENAI_MODEL                                model (default gpt-4o-mini)
//
// As in the weil_adk examples, an agent called name reads
// WEIL_IDENTITY_<KEY> or WEIL_WALLET_<KEY>, <KEY> being name upper-cased with
// every character other than a letter or digit turned into "_".
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	weilmcp "github.com/weilliptic-public/wadk/adk/go/weil_mcp"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

const (
	agentName    = "secured-audit-starter-client"
	mcpServerURL = "http://127.0.0.1:8001/mcp"
	mcpHost      = "127.0.0.1"
	mcpPort      = 8001
	scope        = "engg.weil"
	defaultModel = "gpt-4o-mini"
)

// ---- server side ----

type addInput struct {
	A int `json:"a"`
	B int `json:"b"`
}

// addNumbers returns json.dumps({"a": a, "b": b, "sum": a + b}).
func addNumbers(_ context.Context, _ *mcp.CallToolRequest, in addInput) (*mcp.CallToolResult, any, error) {
	return text(fmt.Sprintf(`{"a": %d, "b": %d, "sum": %d}`, in.A, in.B, in.A+in.B)), nil, nil
}

type shoutInput struct {
	Text string `json:"text"`
}

func shout(_ context.Context, _ *mcp.CallToolRequest, in shoutInput) (*mcp.CallToolResult, any, error) {
	return text(strings.ToUpper(in.Text)), nil, nil
}

func text(s string) *mcp.CallToolResult {
	return &mcp.CallToolResult{Content: []mcp.Content{&mcp.TextContent{Text: s}}}
}

func runServer() {
	server := mcp.NewServer(&mcp.Implementation{Name: "secured-audit-starter", Version: "1.0.0"}, nil)
	// @mcp.tool / @secured(SCOPE) / @audit()
	mcp.AddTool(server, &mcp.Tool{Name: "add_numbers"}, weilmcp.SecuredUser(scope, weilmcp.Audit("", addNumbers)))
	mcp.AddTool(server, &mcp.Tool{Name: "shout"}, weilmcp.SecuredUser(scope, weilmcp.Audit("", shout)))

	handler := mcp.NewStreamableHTTPHandler(func(*http.Request) *mcp.Server { return server }, nil)
	mux := http.NewServeMux()
	// weil_middleware(), on the same Sentinel as the client (see
	// sentinelOptions): checking there keeps the permission lookups on the
	// platform the client's grants live on.
	opts := []secured.Option{secured.WithSentinel(sentinelOptions()...)}
	mux.Handle("/mcp", weilmcp.Middleware(handler, opts...))
	addr := fmt.Sprintf("%s:%d", mcpHost, mcpPort)
	log.Printf("secured-audit-starter MCP server on http://%s/mcp", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}

// ---- client side ----

// myAgent is MyAgent: a chat model bound to the MCP server's tools that
// runs the first tool call it makes.
type myAgent struct {
	model, apiKey, baseURL string
}

func (a *myAgent) run(ctx context.Context, session *mcp.ClientSession, query string) (string, error) {
	listed, err := session.ListTools(ctx, nil)
	if err != nil {
		return "", err
	}
	tools := make([]map[string]any, 0, len(listed.Tools))
	for _, t := range listed.Tools {
		tools = append(tools, map[string]any{
			"type": "function",
			"function": map[string]any{
				"name":        t.Name,
				"description": t.Description,
				"parameters":  t.InputSchema,
			},
		})
	}
	msg, err := a.chat(ctx, map[string]any{
		"model":               a.model,
		"temperature":         0,
		"messages":            []map[string]any{{"role": "user", "content": query}},
		"tools":               tools,
		"parallel_tool_calls": false,
	})
	if err != nil {
		return "", err
	}
	if len(msg.ToolCalls) == 0 {
		return msg.Content, nil
	}

	call := msg.ToolCalls[0].Function
	var args map[string]any
	if err := json.Unmarshal([]byte(call.Arguments), &args); err != nil {
		return "", err
	}
	res, err := session.CallTool(ctx, &mcp.CallToolParams{Name: call.Name, Arguments: args})
	if err != nil {
		return "", err
	}
	var parts []string
	for _, c := range res.Content {
		if t, ok := c.(*mcp.TextContent); ok {
			parts = append(parts, t.Text)
		}
	}
	// A denied secured tool's result has IsError set. Python's client gets
	// the denial text back as the tool's result, prints it and goes on to
	// the next query, so the text is returned either way.
	return strings.Join(parts, "\n"), nil
}

type chatMessage struct {
	Content   string `json:"content"`
	ToolCalls []struct {
		Function struct {
			Name      string `json:"name"`
			Arguments string `json:"arguments"`
		} `json:"function"`
	} `json:"tool_calls"`
}

// chat posts a Chat Completions request and returns the reply's message.
func (a *myAgent) chat(ctx context.Context, body map[string]any) (*chatMessage, error) {
	payload, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, strings.TrimSuffix(a.baseURL, "/")+"/chat/completions", bytes.NewReader(payload))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+a.apiKey)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode >= 400 {
		return nil, fmt.Errorf("openai: %s: %s", resp.Status, strings.TrimSpace(string(data)))
	}
	var out struct {
		Choices []struct {
			Message chatMessage `json:"message"`
		} `json:"choices"`
	}
	if err := json.Unmarshal(data, &out); err != nil {
		return nil, err
	}
	if len(out.Choices) == 0 {
		return nil, errors.New("openai: no choices")
	}
	return &out.Choices[0].Message, nil
}

// buildAgent is @weil_ai.agent(identity=..., verify=VERIFY_TLS) on MyAgent:
// the agent and the wallet it acts under.
func buildAgent(ctx context.Context) (*myAgent, *client.WeilClient, error) {
	c, err := agentWallet(ctx, agentName, sentinelOptions()...)
	if err != nil {
		return nil, nil, err
	}
	model := os.Getenv("OPENAI_MODEL")
	if model == "" {
		model = defaultModel
	}
	baseURL := os.Getenv("OPENAI_BASE_URL")
	if baseURL == "" {
		baseURL = "https://api.openai.com/v1"
	}
	return &myAgent{model: model, apiKey: os.Getenv("OPENAI_API_KEY"), baseURL: baseURL}, c, nil
}

// agentWallet returns the wallet the agent called name acts under:
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), <KEY> being envKey(name).
func agentWallet(ctx context.Context, name string, opts ...client.Option) (*client.WeilClient, error) {
	key := envKey(name)
	var (
		c   *client.WeilClient
		err error
	)
	switch {
	case os.Getenv("WEIL_IDENTITY_"+key) != "":
		c, err = client.FromIdentity(ctx, os.Getenv("WEIL_IDENTITY_"+key), opts...)
	case os.Getenv("WEIL_WALLET_"+key) != "":
		c, err = client.FromWalletFile(os.Getenv("WEIL_WALLET_"+key), opts...)
	default:
		return nil, fmt.Errorf("agent %s: set WEIL_IDENTITY_%s (or WEIL_WALLET_%s)", name, key, key)
	}
	if err != nil {
		return nil, fmt.Errorf("identity of agent %s: %w", name, err)
	}
	return c, nil
}

// envKey is name upper-cased, with every character other than a letter or
// digit turned into "_" (secured-audit-starter-client →
// SECURED_AUDIT_STARTER_CLIENT).
func envKey(name string) string {
	var b strings.Builder
	for _, r := range strings.ToUpper(name) {
		if ('A' <= r && r <= 'Z') || ('0' <= r && r <= '9') {
			b.WriteRune(r)
		} else {
			b.WriteByte('_')
		}
	}
	return b.String()
}

// weilSession is agent.weil_session(session): after every tools/call, the
// audit entry in the result's _meta is submitted to the auditor applet from
// the agent's wallet. A failed submission does not fail the call
// (raise_audit_errors=False).
func weilSession(c *client.WeilClient) mcp.Middleware {
	return func(next mcp.MethodHandler) mcp.MethodHandler {
		return func(ctx context.Context, method string, req mcp.Request) (mcp.Result, error) {
			res, err := next(ctx, method, req)
			if err != nil || method != "tools/call" {
				return res, err
			}
			if result, ok := res.(*mcp.CallToolResult); ok && result != nil {
				if entry, _ := result.Meta[weilmcp.AuditField].(string); entry != "" {
					if _, aerr := c.AuditContext(ctx, entry, client.ExecuteOptions{NonBlocking: true}); aerr != nil {
						log.Printf("audit failed: %v", aerr)
					}
				}
			}
			return res, nil
		}
	}
}

func runClient(ctx context.Context) error {
	agent, wallet, err := buildAgent(ctx)
	if err != nil {
		return err
	}

	mcpClient := mcp.NewClient(&mcp.Implementation{Name: agentName, Version: "1.0.0"}, nil)
	mcpClient.AddSendingMiddleware(weilSession(wallet))
	// Every request carries fresh X-Wallet-* headers signed by the wallet.
	transport := &mcp.StreamableClientTransport{Endpoint: mcpServerURL, HTTPClient: weilmcp.HTTPClient(wallet)}
	session, err := mcpClient.Connect(ctx, transport, nil) // includes initialize
	if err != nil {
		return err
	}
	defer session.Close()

	for _, query := range []string{
		"Use the add_numbers tool to add 2 and 3.",
		"Use the shout tool to uppercase hello from weil.",
	} {
		out, err := agent.run(ctx, session, query)
		if err != nil {
			return err
		}
		fmt.Println(out)
	}
	return nil
}

func main() {
	if len(os.Args) != 2 || (os.Args[1] != "server" && os.Args[1] != "client") {
		fmt.Fprintf(os.Stderr, "usage: %s [-h] {server,client}\n", os.Args[0])
		if len(os.Args) < 2 {
			fmt.Fprintf(os.Stderr, "%s: error: the following arguments are required: mode\n", os.Args[0])
		} else {
			fmt.Fprintf(os.Stderr, "%s: error: argument mode: invalid choice: '%s' (choose from 'server', 'client')\n", os.Args[0], os.Args[1])
		}
		os.Exit(2)
	}
	if os.Args[1] == "server" {
		runServer()
		return
	}
	if err := runClient(context.Background()); err != nil {
		log.Fatal(err)
	}
}
