// secure-travel is the Go version of the Python SDK's weil-travel-a2a example
// (python/examples/google-adk/weil-travel-a2a/{flights,hotels,planner}): three
// agents that talk over secured A2A, in one program.
//
//   - flight_agent (wallet flight-agent) is served on 127.0.0.1:8001 and
//     hotel_agent (wallet hotel-agent) on 127.0.0.1:8002. Each serves only
//     wallets holding Execution or Management in engg.weil, and each one's
//     search tool is SecuredUser("engg.weil").
//   - travel_planner (wallet travel-planner) delegates to both through two
//     remote agents, each with its own wallet that signs its requests:
//     flight_agent (wallet travel-planner-flights) and hotel_agent (wallet
//     travel-planner-hotels).
//
// Every agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent                            identity                <KEY>
//	travel_planner                   travel-planner          TRAVEL_PLANNER
//	flight_agent remote (planner's)  travel-planner-flights  TRAVEL_PLANNER_FLIGHTS
//	hotel_agent remote (planner's)   travel-planner-hotels   TRAVEL_PLANNER_HOTELS
//	flight_agent (server)            flight-agent            FLIGHT_AGENT
//	hotel_agent (server)             hotel-agent             HOTEL_AGENT
//
// The remote agents' wallets need Execution in engg.weil to get past the
// service gates. Inside each server, the secured tool checks that server's
// own wallet, as in the Python SDK, so flight-agent and hotel-agent need it
// too. travel-planner itself needs no grant:
//
//	set_permissions -n weil -g engg -p Execution -s '["<travel-planner-flights address>"]'
//	set_permissions -n weil -g engg -p Execution -s '["<travel-planner-hotels address>"]'
//	set_permissions -n weil -g engg -p Execution -s '["<flight-agent address>"]'
//	set_permissions -n weil -g engg -p Execution -s '["<hotel-agent address>"]'
//
// As Python's setup_secure_a2a does, every root agent gets the Pensieve tools,
// the servers audit and trace through weiladk.NewExecutor, and the planner's
// runner through weiladk.WithSecureA2A.
//
// The model is Claude; the Python agents use LiteLlm openai/gpt-4o.
package main

import (
	"context"
	"fmt"
	"log"
	"net"
	"net/http"
	"time"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/remoteagent/v2"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

const (
	flightsAddr = "127.0.0.1:8001"
	hotelsAddr  = "127.0.0.1:8002"
	scope       = "engg.weil" // subgroup "engg" of org "weil"
)

// agentIdentity returns the identity of the agent called name (see
// devnet.Agent): WEIL_IDENTITY_<KEY>, WEIL_WALLET_<KEY>, else
// ~/.weilliptic/agents/<name>.wc. Sentinel is WEIL_SENTINEL_HOST (default: the
// dev platform); its TLS certificate is verified except for a local platform.
func agentIdentity(ctx context.Context, name string) *weiladk.Identity {
	id, err := devnet.Agent(ctx, name)
	if err != nil {
		log.Fatal(err)
	}
	return id
}

// ═══ Server: flight_agent ═══════════════════════════════════════════════

type FlightArgs struct {
	Origin      string `json:"origin"`
	Destination string `json:"destination"`
	Date        string `json:"date"`
}

// searchFlights is gated by weiladk.SecuredUser below: it runs only when
// flight-agent holds Execution or Management in engg.weil.
func searchFlights(_ agent.Context, in FlightArgs) (string, error) {
	return fmt.Sprintf(`
Available flights from %s to %s on %s:

1. Air India
   Departure: 08:30
   Arrival: 10:45
   Price: Rs 7,200

2. IndiGo
   Departure: 13:15
   Arrival: 15:30
   Price: Rs 6,400

3. Vistara
   Departure: 18:20
   Arrival: 20:35
   Price: Rs 8,100
`, in.Origin, in.Destination, in.Date), nil
}

// ═══ Server: hotel_agent ════════════════════════════════════════════════

type HotelArgs struct {
	City     string `json:"city"`
	CheckIn  string `json:"check_in"`
	CheckOut string `json:"check_out"`
}

// searchHotels is gated by weiladk.SecuredUser below: it runs only when
// hotel-agent holds Execution or Management in engg.weil.
func searchHotels(_ agent.Context, in HotelArgs) (string, error) {
	return fmt.Sprintf(`
Hotels in %s from %s to %s:

1. The Grand Hotel
   Rating: 4.6
   Price: Rs 5,500/night
   Location: City Center

2. Hotel Central
   Rating: 4.3
   Price: Rs 3,800/night
   Location: Near Railway Station

3. The Royal Palace
   Rating: 4.8
   Price: Rs 7,200/night
   Location: Downtown
`, in.City, in.CheckIn, in.CheckOut), nil
}

// serveAgent starts an agent behind SecureA2A on addr and returns once it is
// listening: Python's setup_secure_a2a(service="engg.weil") plus
// to_secure_a2a(host="127.0.0.1", port=...).
func serveAgent(addr string, id *weiladk.Identity, cfg llmagent.Config) {
	cfg.Tools = append(cfg.Tools, weiladk.PensieveTools()...) // setup_secure_a2a adds Pensieve
	served, err := weiladk.NewLLMAgent(id, cfg)
	if err != nil {
		log.Fatalf("create %s: %v", cfg.Name, err)
	}

	// The executor runs the agent for each A2A request. weiladk.NewExecutor
	// takes adka2a's ExecutorConfig unchanged; it refuses requests that did
	// not pass SecureA2A, passes the verified caller to tools and hooks, and
	// audits and traces each run.
	executor, err := weiladk.NewExecutor(adka2a.ExecutorConfig{
		RunnerConfig: runner.Config{
			AppName:        served.Name(),
			Agent:          served,
			SessionService: session.InMemoryService(),
		},
	}, weiladk.WithService(scope))
	if err != nil {
		log.Fatalf("create executor: %v", err)
	}

	// SecureA2A guards the A2A handler: it verifies the signed headers (401)
	// and checks the caller's permission in engg.weil (403) before handing
	// the request to the executor.
	secure, err := weiladk.SecureA2A(
		a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)),
		served, // the served agent: its wallet signs the audit events
		weiladk.WithService(scope),
	)
	if err != nil {
		log.Fatalf("secure handler: %v", err)
	}

	baseURL := "http://" + addr
	card := &a2a.AgentCard{ // the card Python's to_a2a builds
		Name:        served.Name(),
		Description: served.Description(),
		Version:     "0.0.1",
		SupportedInterfaces: []*a2a.AgentInterface{{
			URL:             baseURL + "/",
			ProtocolBinding: a2a.TransportProtocolJSONRPC,
			ProtocolVersion: a2a.Version,
		}},
		DefaultInputModes:  []string{"text/plain"},
		DefaultOutputModes: []string{"text/plain"},
		Skills:             adka2a.BuildAgentSkills(served),
		Capabilities:       a2a.AgentCapabilities{Streaming: true},
	}

	mux := http.NewServeMux()
	mux.Handle(a2asrv.WellKnownAgentCardPath, a2asrv.NewStaticAgentCardHandler(card)) // GET: public card
	mux.Handle("/", secure)                                                           // POST: guarded A2A calls

	listener, err := net.Listen("tcp", addr)
	if err != nil {
		log.Fatalf("listen %s: %v", addr, err)
	}
	go func() { log.Printf("%s stopped: %v", served.Name(), http.Serve(listener, mux)) }()
	log.Printf("%s (wallet %s) on %s", served.Name(), id.Address(), baseURL)
}

func serveFlightAgent(llm model.LLM, id *weiladk.Identity) {
	search, err := functiontool.New(functiontool.Config{Name: "search_flights"},
		weiladk.SecuredUser(scope, searchFlights)) // Execution or Management in engg.weil
	if err != nil {
		log.Fatalf("create search_flights: %v", err)
	}
	serveAgent(flightsAddr, id, llmagent.Config{
		Name:        "flight_agent",
		Model:       llm,
		Description: "Finds flights between cities.",
		Instruction: `
You are a flight search specialist.

When the user asks about flights, use the search_flights tool.
Return the available flights clearly.
Do not answer questions outside your expertise.
Your memory tools hold only your own notes, not the travel planner's: keep
your own records there, and do not look for or update the planner's.
`,
		Tools: []tool.Tool{search},
	})
}

func serveHotelAgent(llm model.LLM, id *weiladk.Identity) {
	search, err := functiontool.New(functiontool.Config{Name: "search_hotels"},
		weiladk.SecuredUser(scope, searchHotels)) // Execution or Management in engg.weil
	if err != nil {
		log.Fatalf("create search_hotels: %v", err)
	}
	serveAgent(hotelsAddr, id, llmagent.Config{
		Name:        "hotel_agent",
		Model:       llm,
		Description: "Finds hotels and accommodation.",
		Instruction: `
You are a hotel search specialist.

When the user asks about accommodation, use the search_hotels tool.
Return useful hotel options with prices and ratings.
Do not answer questions outside your expertise.
Your memory tools hold only your own notes, not the travel planner's: keep
your own records there, and do not look for or update the planner's.
`,
		Tools: []tool.Tool{search},
	})
}

// ═══ Client: travel_planner ═════════════════════════════════════════════

func main() {
	ctx := context.Background()

	llm := devnet.Claude() // API_KEY, else ANTHROPIC_API_KEY; CLAUDE_MODEL
	plannerID := agentIdentity(ctx, "travel-planner")

	serveFlightAgent(llm, agentIdentity(ctx, "flight-agent"))
	serveHotelAgent(llm, agentIdentity(ctx, "hotel-agent"))

	// The remote agents, as agents the planner can hand work to. Each has a
	// wallet of its own, which signs every request it sends.
	remote := func(id *weiladk.Identity, name, description, addr string) agent.Agent {
		a, err := weiladk.NewA2A(id, remoteagent.A2AConfig{
			Name:              name,
			Description:       description,
			AgentCardProvider: remoteagent.NewAgentCardProvider("http://" + addr),
		})
		if err != nil {
			log.Fatalf("create remote %s: %v", name, err)
		}
		return a
	}

	planner, err := weiladk.NewLLMAgent(plannerID, llmagent.Config{
		Name:  "travel_planner",
		Model: llm,
		Instruction: `
You are a travel planning agent.

You have access to two specialist agents:

1. flight_agent
   - Searches for flights.

2. hotel_agent
   - Searches for hotels.

When the user asks you to plan a trip:

1. Determine the destination, origin and dates.
2. Ask flight_agent to find suitable flights.
3. Ask hotel_agent to find suitable hotels.
4. Combine their responses.
5. Present a concise travel plan to the user.

You MUST delegate flight-related work to flight_agent.
You MUST delegate hotel-related work to hotel_agent.
Do not pretend to have searched for flights or hotels yourself.
`,
		SubAgents: []agent.Agent{
			remote(agentIdentity(ctx, "travel-planner-flights"), "flight_agent", "Searches for flights.", flightsAddr),
			remote(agentIdentity(ctx, "travel-planner-hotels"), "hotel_agent", "Searches for hotels.", hotelsAddr),
		},
		Tools: weiladk.PensieveTools(), // setup_secure_a2a adds Pensieve
	})
	if err != nil {
		log.Fatalf("create travel_planner: %v", err)
	}

	// planner.setup_secure_a2a(): agent and secured-tool audits plus a trace
	// of each run, on the calling side.
	secureA2A, err := weiladk.NewPlugin(plugin.Config{},
		weiladk.WithSecureA2A(weiladk.SecureA2AConfig{}),
		weiladk.WithFlushAfterRun(time.Minute))
	if err != nil {
		log.Fatalf("create plugin: %v", err)
	}

	r, err := runner.New(runner.Config{
		AppName:           "travel",
		Agent:             planner,
		SessionService:    session.InMemoryService(),
		AutoCreateSession: true,
		PluginConfig:      runner.PluginConfig{Plugins: []*plugin.Plugin{secureA2A}},
	})
	if err != nil {
		log.Fatalf("create runner: %v", err)
	}

	question := genai.NewContentFromText(
		"I want to travel from Delhi to Bombay from October 20 to October 23. Plan the trip for me.", genai.RoleUser)

	for event, err := range r.Run(ctx, "traveller-1", "trip-1", question, agent.RunConfig{}) {
		if err != nil {
			log.Fatalf("run: %v", err)
		}
		if event.ErrorMessage != "" { // e.g. a remote agent refused the call (401/403)
			fmt.Printf("%s: error: %s\n", event.Author, event.ErrorMessage)
		}
		if event.Content == nil || event.Partial {
			continue
		}
		for _, part := range event.Content.Parts {
			switch {
			case part.FunctionCall != nil:
				fmt.Printf("%s → %s(%v)\n", event.Author, part.FunctionCall.Name, part.FunctionCall.Args)
			case part.FunctionResponse != nil:
				fmt.Printf("%s ← %s: %v\n", event.Author, part.FunctionResponse.Name, part.FunctionResponse.Response)
			case part.Text != "":
				fmt.Printf("%s: %s\n", event.Author, part.Text)
			}
		}
	}
}
