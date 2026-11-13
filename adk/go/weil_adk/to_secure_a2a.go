package weiladk

import (
	"net/http"
	"strings"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/artifact"
	"google.golang.org/adk/v2/memory"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"
)

// ToSecureA2A returns the HTTP handler serving served over secured A2A at
// baseURL (e.g. "http://127.0.0.1:8001") — Python's
// agent.to_secure_a2a(host=..., port=...):
//
//   - the agent card Python's to_a2a builds (version "0.0.1", streaming,
//     text/plain, the agent's skills, "An ADK Agent" when it has no
//     description) at /.well-known/agent-card.json;
//   - JSON-RPC at "/", behind SecureA2A, run by NewExecutor (agent and
//     secured-tool audits, run traces) with in-memory session, artifact and
//     memory services, as to_a2a uses.
//
// opts apply to both SecureA2A and NewExecutor, e.g. WithService. Serve it
// with http.ListenAndServe.
func ToSecureA2A(served agent.Agent, baseURL string, opts ...A2AOption) (http.Handler, error) {
	executor, err := NewExecutor(adka2a.ExecutorConfig{RunnerConfig: runner.Config{
		AppName:         served.Name(),
		Agent:           served,
		SessionService:  session.InMemoryService(),
		ArtifactService: artifact.InMemoryService(),
		MemoryService:   memory.InMemoryService(),
	}}, opts...)
	if err != nil {
		return nil, err
	}
	secure, err := SecureA2A(a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)), served, opts...)
	if err != nil {
		return nil, err
	}
	description := served.Description()
	if description == "" {
		description = "An ADK Agent"
	}
	card := &a2a.AgentCard{
		Name:        served.Name(),
		Description: description,
		Version:     "0.0.1",
		SupportedInterfaces: []*a2a.AgentInterface{{
			URL:             strings.TrimRight(baseURL, "/") + "/",
			ProtocolBinding: a2a.TransportProtocolJSONRPC,
			ProtocolVersion: a2a.Version,
		}},
		DefaultInputModes:  []string{"text/plain"},
		DefaultOutputModes: []string{"text/plain"},
		Skills:             adka2a.BuildAgentSkills(served),
		Capabilities:       a2a.AgentCapabilities{Streaming: true},
	}
	mux := http.NewServeMux()
	mux.Handle(a2asrv.WellKnownAgentCardPath, a2asrv.NewStaticAgentCardHandler(card))
	mux.Handle("/", secure)
	return mux, nil
}
