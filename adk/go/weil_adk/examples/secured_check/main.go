// Secured check: the whole secured stack against a live platform, with a
// scripted model so it needs no LLM key. It reports what the chain says at
// every step:
//
//  1. agent identity lookup (Sentinel's /get_agent_wallet field);
//
//  2. on-chain permissions of each wallet on engg.weil;
//
//  3. a flights agent served behind SecureA2A + a SecuredUser tool, called by
//     a planner and a stranger over A2A — accepted or refused per grant;
//
//  4. a forged and a replayed request, refused before the agent;
//
//  5. the @secured verdicts the server published to the security topic;
//
//  6. Pensieve memory written and read back as the planner.
//
//     go run ./examples/secured_check
//
// Every agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent                             identity        <KEY>
//	flight_agent (served)             flight-agent    FLIGHT_AGENT
//	flight_agent remote (planner's)   travel-planner  TRAVEL_PLANNER
//	flight_agent remote (stranger's)  stranger        STRANGER
//
// Grant travel-planner (it passes the service gate) and flight-agent (its
// secured search_flights tool authorizes as the served agent) to see the
// accepted path: key engg_<wallet> with purpose Execution on identity::weil
// (CLI: set_permissions). The stranger is meant to stay ungranted.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"iter"
	"net"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/remoteagent/v2"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

const service = "engg.weil"

func main() {
	ctx := context.Background()
	opts := devnet.ClientOptions()

	devnet.Section("1. agent identity lookup")
	_, err := client.FromIdentity(ctx, "weil_not-a-real-identity", opts...)
	fmt.Println("FromIdentity(bogus):", err)

	planner := devnet.Must(devnet.Agent(ctx, "travel-planner"))
	stranger := devnet.Must(devnet.Agent(ctx, "stranger"))
	flights := devnet.Must(devnet.Agent(ctx, "flight-agent"))

	devnet.Section("2. on-chain permissions on " + service)
	checker := secured.NewChecker(secured.WithSentinel(opts...), secured.WithoutVerdicts())
	for _, w := range []struct {
		name string
		id   *weiladk.Identity
	}{{"planner", planner}, {"stranger", stranger}, {"flight-agent", flights}} {
		ok, err := checker.Allowed(ctx, w.id.Address(), service, secured.User)
		fmt.Printf("%-13s %s  allowed=%v %v\n", w.name, w.id.Address(), ok, errText(err))
	}

	devnet.Section("3. A2A: flights agent behind SecureA2A, SecuredUser tool")
	url := serveFlights(ctx, flights)
	for _, c := range []struct {
		name string
		id   *weiladk.Identity
	}{{"planner", planner}, {"stranger", stranger}} {
		remote := devnet.Must(weiladk.NewA2A(c.id, remoteagent.A2AConfig{Name: "flight_agent",
			AgentCardProvider: remoteagent.NewAgentCardProvider(url)}))
		fmt.Printf("%-9s → %s\n", c.name, clip(ask(ctx, remote, "flights DEL to BOM"), 220))
	}

	devnet.Section("4. forged and replayed requests")
	body := `{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m-1","role":"ROLE_USER","parts":[{"text":"hi"}]}}}`
	h, _ := planner.Client().AuthHeadersForBody(ctx, client.SignedBody([]byte(body)))
	fmt.Println("first send     :", post(url, h, body))
	fmt.Println("replayed       :", post(url, h, body))
	h2, _ := planner.Client().AuthHeadersForBody(ctx, client.SignedBody([]byte(body)))
	fmt.Println("tampered body  :", post(url, h2, strings.Replace(body, `"hi"`, `"pay me"`, 1)))
	h3, _ := stranger.Client().AuthHeadersForBody(ctx, client.SignedBody([]byte(body)))
	h3[client.HeaderWalletAddress] = planner.Client().PublicKeyAddress()
	fmt.Println("claims planner :", post(url, h3, body))

	devnet.Section("5. verdicts and audit")
	_ = weiladk.DefaultDispatcher().Flush(ctx)
	fmt.Printf("audit dispatcher: %+v\n", weiladk.DefaultDispatcher().Stats())
	time.Sleep(5 * time.Second) // verdicts publish in the background
	res, err := flights.Invoke(ctx, weiladk.Call{Applet: weiladk.Systemic("consumer"), Method: "read_batch",
		Args: map[string]any{"topic": secured.VerdictTopic, "from_seq": 0, "max_count": 200}})
	if err != nil {
		fmt.Println("read", secured.VerdictTopic+":", err)
	} else {
		var entries []struct {
			Seq       uint64 `json:"seq"`
			Publisher string `json:"publisher"`
			Payload   struct {
				Inline string `json:"Inline"`
			} `json:"payload"`
		}
		_ = res.Decode(&entries)
		fmt.Printf("%s holds %d verdicts; latest:\n", secured.VerdictTopic, len(entries))
		for _, e := range entries[max(0, len(entries)-4):] {
			fmt.Printf("  seq %d by %s…: %s\n", e.Seq, e.Publisher[:12], clip(e.Payload.Inline, 120))
		}
	}

	devnet.Section("6. Pensieve memory as the planner")
	p := planner.Client().Pensieve()
	out, err := p.RememberEntity(ctx, planner.Address(), "self", "person", []string{"prefers aisle seats"})
	fmt.Println("remember_entity:", js(out), errText(err))
	out, err = p.RecallEntity(ctx, planner.Address(), "self")
	fmt.Println("recall_entity  :", clip(js(out), 200), errText(err))
}

// serveFlights serves a flights agent the ADK way, secured with weiladk.
func serveFlights(ctx context.Context, id *weiladk.Identity) string {
	search := devnet.Must(functiontool.New(functiontool.Config{Name: "search_flights", Description: "Finds flights."},
		weiladk.SecuredUser(service, func(ctx agent.Context, in struct {
			Origin      string `json:"origin"`
			Destination string `json:"destination"`
		}) (map[string]any, error) {
			caller, _ := weiladk.CallerFromContext(ctx)
			return map[string]any{"flights": []string{"AI-101 " + in.Origin + "→" + in.Destination}, "served_to": caller.Address}, nil
		})))
	served := devnet.Must(weiladk.NewLLMAgent(id, llmagent.Config{Name: "flight_agent", Description: "Finds flights.",
		Model: scripted{}, Tools: []tool.Tool{search}}))
	ln := devnet.Must(net.Listen("tcp", "127.0.0.1:0"))
	base := "http://" + ln.Addr().String()
	card := &a2a.AgentCard{Name: "flight_agent", Description: "Finds flights.", Version: "1.0.0",
		SupportedInterfaces: []*a2a.AgentInterface{{URL: base + "/", ProtocolBinding: a2a.TransportProtocolJSONRPC, ProtocolVersion: a2a.Version}},
		DefaultInputModes:   []string{"text/plain"}, DefaultOutputModes: []string{"text/plain"}}
	executor := devnet.Must(weiladk.NewExecutor(adka2a.ExecutorConfig{RunnerConfig: runner.Config{
		AppName: "flights", Agent: served, SessionService: session.InMemoryService()}}))
	secure := devnet.Must(weiladk.SecureA2A(a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)), served,
		weiladk.WithService(service)))
	mux := http.NewServeMux()
	mux.Handle(a2asrv.WellKnownAgentCardPath, a2asrv.NewStaticAgentCardHandler(card))
	mux.Handle("/", secure)
	go func() { _ = http.Serve(ln, mux) }()
	fmt.Println("flight_agent at", base, "wallet", id.Address())
	return base
}

// scripted calls search_flights once, then reports what it returned.
type scripted struct{}

func (scripted) Name() string { return "scripted" }
func (scripted) GenerateContent(_ context.Context, req *model.LLMRequest, _ bool) iter.Seq2[*model.LLMResponse, error] {
	return func(yield func(*model.LLMResponse, error) bool) {
		for _, c := range req.Contents {
			for _, p := range c.Parts {
				if p.FunctionResponse != nil {
					yield(&model.LLMResponse{Content: genai.NewContentFromText("tool said: "+js(p.FunctionResponse.Response), genai.RoleModel), TurnComplete: true}, nil)
					return
				}
			}
		}
		yield(&model.LLMResponse{Content: genai.NewContentFromFunctionCall("search_flights",
			map[string]any{"origin": "DEL", "destination": "BOM"}, genai.RoleModel), TurnComplete: true}, nil)
	}
}

func ask(ctx context.Context, remote agent.Agent, prompt string) string {
	r := devnet.Must(runner.New(runner.Config{AppName: "planner", Agent: remote, SessionService: session.InMemoryService(), AutoCreateSession: true}))
	var out strings.Builder
	for ev, err := range r.Run(ctx, "u", fmt.Sprint(time.Now().UnixNano()), genai.NewContentFromText(prompt, genai.RoleUser), agent.RunConfig{}) {
		if err != nil {
			return "error: " + err.Error()
		}
		if ev.ErrorMessage != "" {
			out.WriteString("error: " + ev.ErrorMessage)
		}
		if ev.Content != nil && !ev.Partial {
			for _, p := range ev.Content.Parts {
				out.WriteString(p.Text)
			}
		}
	}
	return out.String()
}

func post(url string, h map[string]string, body string) string {
	req, _ := http.NewRequest(http.MethodPost, url+"/", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	for k, v := range h {
		req.Header.Set(k, v)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return err.Error()
	}
	defer resp.Body.Close()
	return resp.Status
}

func js(v any) string { b, _ := json.Marshal(v); return string(b) }

func clip(s string, n int) string {
	s = strings.ReplaceAll(s, "\n", " ")
	if len(s) > n {
		return s[:n] + "…"
	}
	return s
}

func errText(err error) string {
	if err == nil {
		return ""
	}
	return "(" + err.Error() + ")"
}

var _ = os.Getenv
