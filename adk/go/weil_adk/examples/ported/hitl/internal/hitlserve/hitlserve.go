// Package hitlserve holds what the ported HITL examples share: each agent's
// identity, the model, and serving the agent over secured A2A, the twins of
// the Python examples' @weil_ai.agent(identity=..., verify=False),
// LiteLlm(model="openai/gpt-4o") and weil_root_agent.to_secure_a2a(host, port).
//
// Environment:
//
//	WEIL_IDENTITY_<KEY>  the Agent Registry identity (the Python examples'
//	                     identity="weil_xxxx") of the agent named <name>,
//	                     <KEY> = devnet.EnvKey(name); see Identity
//	WEIL_WALLET_<KEY>    else a wallet.wc file for that agent
//	                     (neither: ~/.weilliptic/agents/<name>.wc)
//	WEIL_SENTINEL_HOST   Sentinel base URL (see internal/devnet)
//	OPENAI_API_KEY       the OpenAI key LiteLlm("openai/gpt-4o") reads
package hitlserve

import (
	"context"
	"errors"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"syscall"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/model/openaimodel"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

// Identity resolves the on-chain identity of the agent called name with
// devnet.Agent (WEIL_IDENTITY_<KEY>, else WEIL_WALLET_<KEY>, else
// ~/.weilliptic/agents/<name>.wc): Python's
// @weil_ai.agent(identity=..., verify=False), one per agent. Python's
// verify=False is devnet.ClientOptions' rule for a local platform only:
// elsewhere Sentinel's TLS certificate is verified.
func Identity(ctx context.Context, name string) *weiladk.Identity {
	id, err := devnet.Agent(ctx, name)
	if err != nil {
		log.Fatalf("Failed to load weilliptic identity: %v", err)
	}
	return id
}

// GPT4o is Python's LiteLlm(model="openai/gpt-4o"). The key comes from
// OPENAI_API_KEY, as with LiteLLM.
func GPT4o(ctx context.Context) model.LLM {
	m, err := devnet.E2EModel(openaimodel.NewModel(ctx, "gpt-4o", &openaimodel.ClientConfig{
		APIKey: os.Getenv("OPENAI_API_KEY"),
	}))
	if err != nil {
		log.Fatalf("Failed to create model: %v", err)
	}
	return m
}

// Main serves a until SIGINT or SIGTERM, exiting on failure.
func Main(a agent.Agent, host string, port int) {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if err := Serve(ctx, a, host, port); err != nil {
		log.Fatalf("A2A server failed: %v", err)
	}
}

// Serve is Python's weil_root_agent.to_secure_a2a(host=host, port=port):
// ADK's to_a2a app (agent card at the well-known path, JSON-RPC at "/",
// in-memory session, artifact and memory services) behind Weil's request
// verification. Like the Python examples, it sets no service gate: any
// caller with a valid wallet signature is served.
func Serve(ctx context.Context, a agent.Agent, host string, port int) error {
	addr := net.JoinHostPort(host, strconv.Itoa(port))
	rpcURL := "http://" + addr + "/"

	// Card, executor (setup_secure_a2a's audits and traces) and the
	// verifying gate: Python's to_secure_a2a.
	mux, err := weiladk.ToSecureA2A(a, "http://"+addr)
	if err != nil {
		return fmt.Errorf("secure a2a: %w", err)
	}

	srv := &http.Server{Addr: addr, Handler: mux}
	go func() {
		<-ctx.Done()
		shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdown)
	}()
	log.Printf("Serving %s over secured A2A on %s", a.Name(), rpcURL)
	if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}
