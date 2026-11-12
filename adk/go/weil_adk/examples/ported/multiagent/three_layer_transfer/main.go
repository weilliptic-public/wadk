// Package main is the Go port of the Python example
// python/examples/adk-ported/multi_agent/three_layer_transfer: a three-level
// agent tree that hands the conversation down and back up with
// transfer_to_agent, served over secured A2A on 127.0.0.1:8005.
//
//   - root_agent (parent): project coordinator; delegates writing, plus the
//     Pensieve tools setup_secure_a2a adds.
//   - writer_agent (child): writes, and delegates translation.
//   - translator_agent (grandchild): translates, then transfers back.
//
// Every agent acts under its own identity, resolved by devnet.Agent from
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet file), else ~/.weilliptic/agents/<name>.wc:
//
//	root_agent        three-layer-transfer-root-agent        WEIL_IDENTITY_THREE_LAYER_TRANSFER_ROOT_AGENT / WEIL_WALLET_THREE_LAYER_TRANSFER_ROOT_AGENT
//	writer_agent      three-layer-transfer-writer-agent      WEIL_IDENTITY_THREE_LAYER_TRANSFER_WRITER_AGENT / WEIL_WALLET_THREE_LAYER_TRANSFER_WRITER_AGENT
//	translator_agent  three-layer-transfer-translator-agent  WEIL_IDENTITY_THREE_LAYER_TRANSFER_TRANSLATOR_AGENT / WEIL_WALLET_THREE_LAYER_TRANSFER_TRANSLATOR_AGENT
//
// Grants: no agent runs a secured tool, so none needs a grant.
//
// Run:
//
//	export OPENAI_API_KEY=...                                     # LiteLlm("openai/gpt-4o")
//	export WEIL_IDENTITY_THREE_LAYER_TRANSFER_ROOT_AGENT=weil_...  # optional, likewise for the others
//	go run ./examples/ported/multiagent/three_layer_transfer              # a2a_app: secured A2A on :8005
//	go run ./examples/ported/multiagent/three_layer_transfer console      # root_agent: adk run
//	go run ./examples/ported/multiagent/three_layer_transfer web api webui # root_agent: adk web
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

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

// Python: a2a_app = weil_root_agent.to_secure_a2a(host="127.0.0.1", port=8005)
const (
	a2aHost = "127.0.0.1"
	a2aPort = 8005
)

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

	// --- Leaf Agent (Grandchild) ---
	translatorAgent, err := weiladk.NewLLMAgent(identity(ctx, "three-layer-transfer-translator-agent"), llmagent.Config{
		Name:        "translator_agent",
		Model:       m,
		Description: "Translates text into different languages.",
		Instruction: `
      You are a translator. Your job is to translate the text provided to you into the requested language.
      Once the translation is complete, output the translated text, explain what you did, and then transfer back to the writer_agent.
    `,
	})
	if err != nil {
		log.Fatalf("Failed to create translator_agent: %v", err)
	}

	// --- Middle Agent (Child) ---
	writerAgent, err := weiladk.NewLLMAgent(identity(ctx, "three-layer-transfer-writer-agent"), llmagent.Config{
		Name:        "writer_agent",
		Model:       m,
		Description: "Writes stories, articles, or essays, and manages translation requests.",
		Instruction: `
      You are a professional writer.
      When asked to write something, perform the writing task and present the result to the user.
      If the user asks to translate the written content into another language, transfer the task to the translator_agent.
      If the user is satisfied and wants to return to the main coordinator, transfer back to the root_agent.
    `,
		SubAgents: []agent.Agent{translatorAgent},
	})
	if err != nil {
		log.Fatalf("Failed to create writer_agent: %v", err)
	}

	// --- Root Agent (Parent) ---
	root, err := weiladk.NewLLMAgent(identity(ctx, "three-layer-transfer-root-agent"), llmagent.Config{
		Name:        "root_agent",
		Model:       m,
		Description: "Project coordinator that delegates writing and translation tasks.",
		Instruction: `
      You are a project coordinator.
      If the user wants to write a story, essay, or article, transfer the task to the writer_agent.
      Answer general inquiries yourself, but delegate writing-related tasks.
    `,
		SubAgents: []agent.Agent{writerAgent},
		// setup_secure_a2a(pensieve=True) adds the Pensieve tools to the root.
		Tools: weiladk.PensieveTools(),
	})
	if err != nil {
		log.Fatalf("Failed to create root_agent: %v", err)
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
