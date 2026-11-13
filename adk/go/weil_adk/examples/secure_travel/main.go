// Secure travel: the Go twin of the Python SDK's weil-travel-a2a example
// (python/examples/google-adk/weil-travel-a2a/{flights,hotels,planner}),
// wire-compatible with it — any agent here can be swapped for its Python
// counterpart.
//
//	Python (weil_ai)                                   Go (weiladk)
//	@weil_ai.agent(identity=...)                       devnet.Agent (weiladk.FromIdentity / FromWalletFile)
//	@weil_ai.secured_user("engg.weil") on a tool       weiladk.SecuredUser("engg.weil", fn)
//	agent.setup_secure_a2a(service="engg.weil")        weiladk.NewExecutor(..., weiladk.WithService("engg.weil"))
//	  + to_secure_a2a(...)                               + weiladk.SecureA2A(handler, agent, weiladk.WithService("engg.weil"))
//	RemoteA2aAgent + planner.setup_secure_a2a()        weiladk.NewA2A(id, remoteagent.A2AConfig{...})
//	                                                     + weiladk.NewPlugin(plugin.Config{}, weiladk.WithSecureA2A(...))
//	setup_secure_a2a() adds the Pensieve tools         weiladk.PensieveTools() in each root agent's Tools
//
// A travel planner delegates to a flights agent (127.0.0.1:8001) and a hotels
// agent (127.0.0.1:8002) over A2A, through one remote agent per server, each
// with a wallet of its own that signs its requests, bound to the message.
// Each server verifies the request and checks on chain that the calling
// wallet holds the Execution (or Management) purpose in the engg subgroup of
// org weil. Inside each server, the secured tool checks that server's own
// wallet, as in the Python SDK, and sees the planner's remote agent as its
// caller. A "stranger" wallet is refused by both. All of it is audited on
// chain by each side.
//
//	go run ./examples/secure_travel
//	go run ./examples/secure_travel -hotels http://127.0.0.1:8002   # a Python hotels agent
//
// Every agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent                            identity                <KEY>                   grant
//	travel_planner                   travel-planner          TRAVEL_PLANNER          -
//	flight_agent remote (planner's)  travel-planner-flights  TRAVEL_PLANNER_FLIGHTS  engg.weil
//	hotel_agent remote (planner's)   travel-planner-hotels   TRAVEL_PLANNER_HOTELS   engg.weil
//	flight_agent (server, :8001)     flight-agent            FLIGHT_AGENT            engg.weil
//	hotel_agent (server, :8002)      hotel-agent             HOTEL_AGENT             engg.weil
//	flight_agent remote (stranger's) stranger                STRANGER                none (refused)
//
// A grant on engg.weil is key engg_<wallet> with purpose Execution on
// identity::weil. The remote agents' wallets pass the servers' gates; the
// servers' own wallets run their secured search tools.
//
// The model is Claude (devnet.Claude); the Python agents use LiteLlm
// openai/gpt-4o.
package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/remoteagent/v2"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

const (
	service     = "engg.weil" // subgroup engg of org weil (FLIGHT_SERVICE, HOTEL_SERVICE)
	flightsAddr = "127.0.0.1:8001"
	hotelsAddr  = "127.0.0.1:8002"
)

// ── flights ──────────────────────────────────────────────────────────────

type FlightArgs struct {
	Origin      string `json:"origin"`
	Destination string `json:"destination"`
	Date        string `json:"date"`
}

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

const flightInstruction = `
You are a flight search specialist.

When the user asks about flights, use the search_flights tool.
Return the available flights clearly.
Do not answer questions outside your expertise.
Your memory tools hold only your own notes, not the travel planner's: keep
your own records there, and do not look for or update the planner's.
`

// ── hotels ───────────────────────────────────────────────────────────────

type HotelArgs struct {
	City     string `json:"city"`
	CheckIn  string `json:"check_in"`
	CheckOut string `json:"check_out"`
}

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

const hotelInstruction = `
You are a hotel search specialist.

When the user asks about accommodation, use the search_hotels tool.
Return useful hotel options with prices and ratings.
Do not answer questions outside your expertise.
Your memory tools hold only your own notes, not the travel planner's: keep
your own records there, and do not look for or update the planner's.
`

// ── planner ──────────────────────────────────────────────────────────────

const plannerInstruction = `
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
`

func main() {
	hotelsURL := flag.String("hotels", "", "use this (e.g. Python) hotels agent instead of serving the Go one on "+hotelsAddr)
	flightsURL := flag.String("flights", "", "use this (e.g. Python) flights agent instead of serving the Go one on "+flightsAddr)
	flag.Parse()
	ctx := context.Background()

	planner := devnet.Must(devnet.Agent(ctx, "travel-planner"))
	toFlights := devnet.Must(devnet.Agent(ctx, "travel-planner-flights")) // the planner's flight_agent remote
	toHotels := devnet.Must(devnet.Agent(ctx, "travel-planner-hotels"))   // the planner's hotel_agent remote
	stranger := devnet.Must(devnet.Agent(ctx, "stranger"))
	fmt.Printf("planner   %s\n→flights  %s\n→hotels   %s\nstranger  %s\n",
		planner.Address(), toFlights.Address(), toHotels.Address(), stranger.Address())

	// Who may call? The same check the servers run.
	devnet.Section("on-chain permissions on " + service)
	checker := secured.NewChecker(secured.WithSentinel(devnet.ClientOptions()...), secured.WithoutVerdicts())
	for name, id := range map[string]*weiladk.Identity{"→flights": toFlights, "→hotels": toHotels, "stranger": stranger} {
		ok, err := checker.Allowed(ctx, id.Address(), service, secured.User)
		fmt.Printf("%-9s allowed=%v %v\n", name, ok, errText(err))
	}

	if *flightsURL == "" {
		searchTool := devnet.Must(functiontool.New(functiontool.Config{Name: "search_flights"},
			weiladk.SecuredUser(service, searchFlights))) // @weil_ai.secured_user("engg.weil")
		*flightsURL = serve(ctx, flightsAddr, "flight-agent", llmagent.Config{
			Name:        "flight_agent",
			Description: "Finds flights between cities.",
			Instruction: flightInstruction,
			Tools:       []tool.Tool{searchTool},
		})
	}
	if *hotelsURL == "" {
		searchTool := devnet.Must(functiontool.New(functiontool.Config{Name: "search_hotels"},
			weiladk.SecuredUser(service, searchHotels))) // @weil_ai.secured_user("engg.weil")
		*hotelsURL = serve(ctx, hotelsAddr, "hotel-agent", llmagent.Config{
			Name:        "hotel_agent",
			Description: "Finds hotels and accommodation.",
			Instruction: hotelInstruction,
			Tools:       []tool.Tool{searchTool},
		})
	}

	// ── The planner: plain ADK sub-agents, each a weiladk.NewA2A with its own wallet ─
	remote := func(id *weiladk.Identity, name, desc, url string) agent.Agent {
		return devnet.Must(weiladk.NewA2A(id, remoteagent.A2AConfig{
			Name: name, Description: desc, AgentCardProvider: remoteagent.NewAgentCardProvider(url),
		}))
	}
	plannerAgent := devnet.Must(weiladk.NewLLMAgent(planner, llmagent.Config{
		Name:        "travel_planner",
		Model:       devnet.Claude(),
		Instruction: plannerInstruction,
		SubAgents: []agent.Agent{
			remote(toFlights, "flight_agent", "Searches for flights.", *flightsURL),
			remote(toHotels, "hotel_agent", "Searches for hotels.", *hotelsURL),
		},
		Tools: weiladk.PensieveTools(), // planner.setup_secure_a2a() adds Pensieve
	}))
	// planner.setup_secure_a2a(): agent and secured-tool audits plus run
	// traces on the calling side.
	secureA2A := devnet.Must(weiladk.NewPlugin(plugin.Config{},
		weiladk.WithSecureA2A(weiladk.SecureA2AConfig{}),
		weiladk.WithFlushAfterRun(time.Minute)))
	devnet.Section("planner plans a trip")
	devnet.Must(devnet.Run(ctx, devnet.NewRunner("travel", plannerAgent,
		runner.PluginConfig{Plugins: []*plugin.Plugin{secureA2A}}), "trip",
		"I want to travel from Delhi to Bombay from October 20 to October 23. Plan the trip for me."))

	devnet.Section("a stranger calls the flights agent directly")
	intruder := remote(stranger, "flight_agent", "Searches for flights.", *flightsURL)
	reply, err := devnet.Run(ctx, devnet.NewRunner("stranger", intruder, runner.PluginConfig{}), "s", "Flights Delhi to Bombay on Oct 20")
	fmt.Println("stranger got:", strings.TrimSpace(reply), errText(err))
	_ = weiladk.DefaultDispatcher().Flush(ctx) // audit events
}

// serve runs one specialist the ADK way — agent, card, executor, a2asrv
// handler — secured with weiladk as Python's setup_secure_a2a(service=...)
// plus to_secure_a2a(host="127.0.0.1", port=...) do, and returns its base
// URL.
func serve(ctx context.Context, addr, wallet string, cfg llmagent.Config) string {
	id := devnet.Must(devnet.Agent(ctx, wallet))
	cfg.Model = devnet.Claude()
	cfg.Tools = append(cfg.Tools, weiladk.PensieveTools()...) // setup_secure_a2a() adds Pensieve
	specialist := devnet.Must(weiladk.NewLLMAgent(id, cfg))

	base := "http://" + addr
	card := &a2a.AgentCard{ // the card Python's to_a2a builds
		Name: specialist.Name(), Description: specialist.Description(), Version: "0.0.1",
		SupportedInterfaces: []*a2a.AgentInterface{{URL: base + "/", ProtocolBinding: a2a.TransportProtocolJSONRPC, ProtocolVersion: a2a.Version}},
		DefaultInputModes:   []string{"text/plain"}, DefaultOutputModes: []string{"text/plain"},
		Skills:       adka2a.BuildAgentSkills(specialist),
		Capabilities: a2a.AgentCapabilities{Streaming: true},
	}
	// NewExecutor installs setup_secure_a2a(service="engg.weil"): agent and
	// secured-tool audits, and run traces.
	executor := devnet.Must(weiladk.NewExecutor(adka2a.ExecutorConfig{RunnerConfig: runner.Config{
		AppName: specialist.Name(), Agent: specialist, SessionService: session.InMemoryService(),
	}}, weiladk.WithService(service)))
	secure := devnet.Must(weiladk.SecureA2A(a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)), specialist,
		weiladk.WithService(service))) // the inbound service gate

	mux := http.NewServeMux()
	mux.Handle(a2asrv.WellKnownAgentCardPath, a2asrv.NewStaticAgentCardHandler(card))
	mux.Handle("/", secure)
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		log.Fatalf("listen %s: %v", addr, err)
	}
	go func() { _ = http.Serve(ln, mux) }()
	fmt.Printf("%-13s %s  wallet %s\n", specialist.Name(), base, id.Address())
	return base
}

func errText(err error) string {
	if err == nil {
		return ""
	}
	return "(" + err.Error() + ")"
}
