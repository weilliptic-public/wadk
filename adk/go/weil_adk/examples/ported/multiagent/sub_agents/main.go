// Package main is the Go port of the Python example
// python/examples/adk-ported/multi_agent/sub_agents: a root agent that checks
// accounts and closes them by delegating to two sub-agents, served over
// secured A2A on 127.0.0.1:8002.
//
//   - sub_agents (root): no tools of its own besides the Pensieve tools
//     setup_secure_a2a adds; delegates by transfer_to_agent.
//   - info_agent: get_account_status, secured to engg.weil.
//   - close_agent: close_account, secured to engg.weil, which asks the user
//     to confirm every call.
//
// Every agent acts under its own identity, resolved by devnet.Agent from
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	sub_agents (root)  sub-agents-root-agent   WEIL_IDENTITY_SUB_AGENTS_ROOT_AGENT / WEIL_WALLET_SUB_AGENTS_ROOT_AGENT
//	info_agent         sub-agents-info-agent   WEIL_IDENTITY_SUB_AGENTS_INFO_AGENT / WEIL_WALLET_SUB_AGENTS_INFO_AGENT
//	close_agent        sub-agents-close-agent  WEIL_IDENTITY_SUB_AGENTS_CLOSE_AGENT / WEIL_WALLET_SUB_AGENTS_CLOSE_AGENT
//
// Grants: secured tools authorize the agent running them, so info_agent's
// wallet (get_account_status) and close_agent's wallet (close_account) each
// need Execution in engg.weil. The root runs no secured tool.
//
// Run:
//
//	export OPENAI_API_KEY=...                          # LiteLlm("openai/gpt-4o")
//	export WEIL_IDENTITY_SUB_AGENTS_ROOT_AGENT=weil_...  # optional, likewise for the others
//	go run ./examples/ported/multiagent/sub_agents              # a2a_app: secured A2A on :8002
//	go run ./examples/ported/multiagent/sub_agents console      # root_agent: adk run
//	go run ./examples/ported/multiagent/sub_agents web api webui # root_agent: adk web
package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"os"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/cmd/launcher"
	"google.golang.org/adk/v2/cmd/launcher/full"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/model/openaimodel"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

// Python: a2a_app = weil_root_agent.to_secure_a2a(host="127.0.0.1", port=8002)
const (
	a2aHost = "127.0.0.1"
	a2aPort = 8002
)

// AccountInput is the argument of both account tools.
type AccountInput struct {
	AccountID string `json:"account_id"`
}

// getAccountStatus gets the status of a bank account.
func getAccountStatus(_ agent.Context, in AccountInput) (string, error) {
	return fmt.Sprintf("Account %s is active.", in.AccountID), nil
}

// closeAccount closes a bank account.
func closeAccount(_ agent.Context, in AccountInput) (string, error) {
	return fmt.Sprintf("Account %s has been closed.", in.AccountID), nil
}

// identity is Python's @weil_ai.agent(identity="weil_xxxx", verify=False),
// per agent: see devnet.Agent. Python's verify=False is
// devnet.ClientOptions' rule for a local platform only: elsewhere Sentinel's
// TLS certificate is verified.
func identity(ctx context.Context, name string) *weiladk.Identity {
	id, err := devnet.Agent(ctx, name)
	if err != nil {
		log.Fatalf("Failed to load agent identity: %v", err)
	}
	return id
}

// newModel is Python's LiteLlm(model="openai/gpt-4o").
func newModel(ctx context.Context) model.LLM {
	m, err := devnet.E2EModel(openaimodel.NewModel(ctx, "gpt-4o", &openaimodel.ClientConfig{APIKey: os.Getenv("OPENAI_API_KEY")}))
	if err != nil {
		log.Fatalf("Failed to create model: %v", err)
	}
	return m
}

func newRootAgent(ctx context.Context) agent.Agent {
	m := newModel(ctx)

	getAccountStatusTool, err := functiontool.New(functiontool.Config{
		Name: "get_account_status",
		Description: `Gets the status of a bank account.

Args:
    account_id: The account ID to check.

Returns:
    The status of the account.`,
	}, weiladk.SecuredUser("engg.weil", getAccountStatus))
	if err != nil {
		log.Fatalf("Failed to create get_account_status: %v", err)
	}

	closeAccountTool, err := functiontool.New(functiontool.Config{
		Name: "close_account",
		Description: `Closes a bank account. This action requires user confirmation.

Args:
    account_id: The account ID to close.

Returns:
    A confirmation message.`,
		RequireConfirmation: true,
	}, weiladk.SecuredUser("engg.weil", closeAccount))
	if err != nil {
		log.Fatalf("Failed to create close_account: %v", err)
	}

	infoAgent, err := weiladk.NewLLMAgent(identity(ctx, "sub-agents-info-agent"), llmagent.Config{
		Name:        "info_agent",
		Model:       m,
		Description: "An agent that can check account status.",
		Tools:       []tool.Tool{getAccountStatusTool},
	})
	if err != nil {
		log.Fatalf("Failed to create info_agent: %v", err)
	}

	closeAgent, err := weiladk.NewLLMAgent(identity(ctx, "sub-agents-close-agent"), llmagent.Config{
		Name:        "close_agent",
		Model:       m,
		Description: "An agent that can close accounts.",
		Tools:       []tool.Tool{closeAccountTool},
	})
	if err != nil {
		log.Fatalf("Failed to create close_agent: %v", err)
	}

	root, err := weiladk.NewLLMAgent(identity(ctx, "sub-agents-root-agent"), llmagent.Config{
		Name:  "sub_agents",
		Model: m,
		Description: "A root agent that can check accounts and close them by delegating to" +
			" sub-agents.",
		SubAgents: []agent.Agent{infoAgent, closeAgent},
		// setup_secure_a2a(pensieve=True) adds the Pensieve tools to the root.
		Tools: weiladk.PensieveTools(),
	})
	if err != nil {
		log.Fatalf("Failed to create sub_agents: %v", err)
	}
	return root
}

func main() {
	ctx := context.Background()
	root := newRootAgent(ctx)

	if len(os.Args) > 1 {
		runLocal(ctx, root)
		return
	}
	if err := serveSecureA2A(root, a2aHost, a2aPort); err != nil {
		log.Fatalf("A2A server stopped: %v", err)
	}
}

// runLocal runs root_agent under the ADK launcher (Python: adk run / adk
// web), with the hooks setup_secure_a2a installs: agent and secured-tool
// audits, and a trace of each run.
func runLocal(ctx context.Context, root agent.Agent) {
	p, err := weiladk.NewPlugin(plugin.Config{},
		weiladk.WithSecureA2A(weiladk.SecureA2AConfig{}),
		weiladk.WithFlushAfterRun(time.Minute))
	if err != nil {
		log.Fatalf("Failed to create plugin: %v", err)
	}
	config := &launcher.Config{
		AgentLoader:  agent.NewSingleLoader(root),
		PluginConfig: runner.PluginConfig{Plugins: []*plugin.Plugin{p}},
	}
	l := full.NewLauncher()
	if err := l.Execute(ctx, config, os.Args[1:]); err != nil {
		log.Fatalf("Run failed: %v\n\n%s", err, l.CommandLineSyntax())
	}
}

// serveSecureA2A is Python's to_secure_a2a(host=..., port=...) served by
// uvicorn: the agent card at /.well-known/agent-card.json, JSON-RPC at /,
// each request's wallet signature verified. setup_secure_a2a() was called
// without a service, so there is no permission gate (no WithService).
func serveSecureA2A(root agent.Agent, host string, port int) error {
	addr := fmt.Sprintf("%s:%d", host, port)
	handler, err := weiladk.ToSecureA2A(root, "http://"+addr)
	if err != nil {
		return fmt.Errorf("secure A2A handler: %w", err)
	}
	log.Printf("Serving %s over secured A2A on http://%s", root.Name(), addr)
	return http.ListenAndServe(addr, handler)
}
