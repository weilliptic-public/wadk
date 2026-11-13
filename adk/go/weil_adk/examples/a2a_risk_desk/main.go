// Agent-to-agent with on-chain identities — written like ADK's own A2A
// example, with each A2A call swapped for its weiladk twin:
//
//	ADK                                    weiladk
//	llmagent.New(cfg)                  →   weiladk.NewLLMAgent(id, cfg)
//	card := &a2a.AgentCard{...}        →   + weiladk.SignAgentCard(ctx, id, card)   (optional)
//	adka2a.NewExecutor(cfg)            →   weiladk.NewExecutor(cfg, opts...)
//	a2asrv.NewJSONRPCHandler(h)        →   wrapped in weiladk.SecureA2A(h, agent, opts...)
//	remoteagent.NewA2A(cfg)            →   weiladk.NewA2A(id, cfg, opts...)
//	plugin.New(cfg)                    →   weiladk.NewPlugin(cfg, opts...)
//
// A "risk_desk" agent is served over A2A; its card also carries its wallet,
// signed with that wallet's key. A "portfolio_manager" agent verifies the
// card and calls the desk as a tool through a remote agent with a wallet of
// its own, which signs every request (the same wire protocol as the Python
// SDK). The desk serves only clients its wallet listed on chain, and both
// sides record each exchange to a shared receipts topic, signed by their own
// wallets. A "stranger" with a valid wallet but no listing is turned away.
//
//	go run ./examples/a2a_risk_desk
//
// Every agent acts under its own identity (devnet.Agent), set per agent by
// WEIL_IDENTITY_<KEY> (an Agent Registry identity) or WEIL_WALLET_<KEY> (a
// wallet.wc file), else ~/.weilliptic/agents/<name>.wc, created on first use:
//
//	agent                          identity               <KEY>
//	risk_desk (A2A server)         risk-desk              RISK_DESK
//	portfolio_manager              portfolio-manager      PORTFOLIO_MANAGER
//	risk_desk remote (PM's caller) portfolio-manager-a2a  PORTFOLIO_MANAGER_A2A
//	risk_desk remote (stranger)    stranger               STRANGER
//
// The vendor-acme and vendor-shellco wallets (VENDOR_ACME, VENDOR_SHELLCO)
// only supply the addresses being assessed. No grants are needed: the desk's
// allowlist is the on-chain topic it writes itself, listing
// portfolio-manager-a2a, the wallet that signs the PM's A2A requests.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"time"

	"github.com/a2aproject/a2a-go/v2/a2a"
	"github.com/a2aproject/a2a-go/v2/a2asrv"
	"github.com/google/jsonschema-go/jsonschema"
	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/agent/remoteagent/v2"
	"google.golang.org/adk/v2/plugin"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/server/adka2a/v2"
	"google.golang.org/adk/v2/session"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/internal/devnet"
)

var (
	publisher = weiladk.Systemic("publisher")
	consumer  = weiladk.Systemic("consumer")
)

func main() {
	ctx := context.Background()
	desk := devnet.Must(devnet.Agent(ctx, "risk-desk"))
	pm := devnet.Must(devnet.Agent(ctx, "portfolio-manager"))
	pmA2A := devnet.Must(devnet.Agent(ctx, "portfolio-manager-a2a")) // signs the PM's requests to the desk
	stranger := devnet.Must(devnet.Agent(ctx, "stranger"))
	acme := devnet.Must(devnet.Agent(ctx, "vendor-acme")).Address()
	shellco := devnet.Must(devnet.Agent(ctx, "vendor-shellco")).Address()
	fmt.Printf("risk desk          %s\nportfolio manager  %s\n  its A2A caller   %s\nstranger           %s\n",
		desk.Address(), pm.Address(), pmA2A.Address(), stranger.Address())

	// The desk lists its clients on chain and opens a shared receipts topic.
	run := time.Now().Unix()
	clients := fmt.Sprintf("risk-desk-clients-%d", run)
	receipts := fmt.Sprintf("a2a-receipts-%d", run)
	for _, topic := range []string{clients, receipts} {
		devnet.Must(devnet.SetupCall(ctx, desk, weiladk.Call{Applet: publisher, Method: "create_topic",
			Args: map[string]any{"topic": topic, "description": "risk desk " + topic, "retention_in_bytes": 1 << 20}}))
	}
	devnet.Must(devnet.SetupCall(ctx, desk, weiladk.Call{Applet: publisher, Method: "publish",
		Args: map[string]any{"topic": clients, "payload": fmt.Sprintf(`{"client":%q}`, pmA2A.Address())}}))
	fmt.Println("client allowlist:", clients, "\nreceipts topic:  ", receipts)

	devnet.Section("starting the risk desk A2A server")
	baseURL := serveRiskDesk(ctx, desk, clients, receipts)
	fmt.Println("risk desk card:", baseURL+a2asrv.WellKnownAgentCardPath)

	// ── Client: remoteagent.NewA2A's twin ───────────────────────────────
	devnet.Section("portfolio manager consults the risk desk over A2A")
	remote := devnet.Must(weiladk.NewA2A(pmA2A, remoteagent.A2AConfig{
		Name:              "risk_desk",
		Description:       "External counterparty-risk desk.",
		AgentCardProvider: remoteagent.NewAgentCardProvider(baseURL),
	}, weiladk.WithExpectedWallet(desk.Address()))) // refuse anyone but the desk's wallet

	pmAgent := devnet.Must(weiladk.NewLLMAgent(pm, llmagent.Config{
		Name:  "portfolio_manager",
		Model: devnet.Claude(),
		Instruction: "You manage payments to counterparties. Before recommending any payment, consult the risk_desk tool " +
			"(an external risk agent) once per counterparty wallet, then give a short recommendation per counterparty.",
		Tools: []tool.Tool{weiladk.NewAgentTool(remote, nil)},
	}))

	// Client half of each receipt, as a plain ADK callback.
	pmPlugin := devnet.Must(weiladk.NewPlugin(plugin.Config{
		Name: "portfolio",
		AfterToolCallback: func(ctx agent.Context, t tool.Tool, args, result map[string]any, err error) (map[string]any, error) {
			if t.Name() != remote.Name() {
				return nil, nil
			}
			callee, _ := weiladk.RemoteIdentity(remote)
			return nil, weiladk.Submit(ctx, receipt(receipts, map[string]any{
				"side": "client", "callee": callee.Wallet, "request": args, "answer_sha256": weiladk.HashJSON(result),
			}), nil)
		},
	}, weiladk.WithFlushAfterRun(time.Minute)))
	r := devnet.NewRunner("portfolio", pmAgent, runner.PluginConfig{Plugins: []*plugin.Plugin{pmPlugin}})
	devnet.Must(devnet.Run(ctx, r, "pm-session", fmt.Sprintf(
		"We are about to pay two counterparties: Acme Supplies (wallet %s) and ShellCo Ltd (wallet %s). Should we?", acme, shellco)))
	_ = pmPlugin.Close()
	if ci, ok := weiladk.RemoteIdentity(remote); ok {
		fmt.Println("verified remote card: risk_desk is wallet", ci.Wallet)
	}

	// ── A stranger: same code, a wallet that is not a client ────────────
	devnet.Section("a stranger calls the risk desk directly")
	intruder := devnet.Must(weiladk.NewA2A(stranger, remoteagent.A2AConfig{
		Name: "risk_desk", AgentCardProvider: remoteagent.NewAgentCardProvider(baseURL),
	}, weiladk.WithExpectedWallet(desk.Address())))
	if reply, _ := devnet.Run(ctx, devnet.NewRunner("stranger", intruder, runner.PluginConfig{}), "stranger-session", "Assess wallet "+acme); reply == "" {
		fmt.Println("(no answer — the desk refused the stranger)")
	}

	// ── Both halves of each exchange, read back from the chain ──────────
	devnet.Section("two-sided receipts on chain (" + receipts + ")")
	res := devnet.Must(devnet.SetupCall(ctx, desk, weiladk.Call{Applet: consumer, Method: "read_batch",
		Args: map[string]any{"topic": receipts, "from_seq": 0, "max_count": 50}}))
	var entries []struct {
		Seq       uint64 `json:"seq"`
		Publisher string `json:"publisher"`
		Block     uint64 `json:"block_height"`
		Payload   struct {
			Inline string `json:"Inline"`
		} `json:"payload"`
	}
	devnet.Must(0, res.Decode(&entries))
	for _, e := range entries {
		var body map[string]any
		_ = json.Unmarshal([]byte(e.Payload.Inline), &body)
		who := map[string]string{desk.Address(): "risk desk", pm.Address(): "portfolio manager"}[e.Publisher]
		fmt.Printf("seq %d  block %d  signed by %-17s side=%v caller=%v callee=%v\n",
			e.Seq, e.Block, who, body["side"], short(body["caller"]), short(body["callee"]))
	}
}

// serveRiskDesk serves the desk exactly like ADK's A2A example — card,
// executor, a2asrv handlers — using the weiladk twins. Returns its base URL.
func serveRiskDesk(ctx context.Context, desk *weiladk.Identity, clients, receipts string) string {
	ln := devnet.Must(net.Listen("tcp", "127.0.0.1:0"))
	baseURL := "http://" + ln.Addr().String()

	addr := &jsonschema.Schema{Type: "object", Properties: map[string]*jsonschema.Schema{
		"addr": {Type: "string", Description: "Wallet address."}}, Required: []string{"addr"}}
	deskAgent := devnet.Must(weiladk.NewLLMAgent(desk, llmagent.Config{
		Name:        "risk_desk",
		Model:       devnet.Claude(),
		Description: "Assesses the counterparty risk of a WeilChain wallet from its on-chain holdings.",
		Instruction: "You are a counterparty-risk desk. For each wallet you are asked about, look up its WUSD balance and its Ledger " +
			"token holdings with your tools, then rate it LOW, MEDIUM or HIGH risk with a one-line reason. A wallet with no holdings " +
			"and no history is HIGH risk. Answer in under 80 words.",
		Tools: []tool.Tool{
			devnet.Must(weiladk.NewAppletTool(weiladk.AppletToolConfig{Name: "wusd_balance", Description: "A wallet's WUSD balance.",
				Applet: weiladk.Systemic("wusd"), Method: "balance_for", InputSchema: addr})),
			devnet.Must(weiladk.NewAppletTool(weiladk.AppletToolConfig{Name: "ledger_holdings", Description: "A wallet's Ledger token balances.",
				Applet: weiladk.Systemic("Ledger"), Method: "balances_for", InputSchema: addr})),
		},
	}))

	// Server half of each receipt: who asked (verified), and a digest of the answer.
	deskPlugin := devnet.Must(weiladk.NewPlugin(plugin.Config{
		Name: "risk-desk",
		AfterAgentCallback: func(ctx agent.Context) (*genai.Content, error) {
			caller, ok := weiladk.CallerFromContext(ctx)
			if !ok {
				return nil, nil
			}
			return nil, weiladk.Submit(ctx, receipt(receipts, map[string]any{
				"side": "server", "caller": caller.Address, "nonce": caller.Nonce,
			}), nil)
		},
	}, weiladk.WithFlushAfterRun(time.Minute)))

	// The agent card, built the ADK way, then signed with the desk's wallet.
	card := &a2a.AgentCard{
		Name:        deskAgent.Name(),
		Description: deskAgent.Description(),
		SupportedInterfaces: []*a2a.AgentInterface{{
			URL: baseURL + "/invoke", ProtocolBinding: a2a.TransportProtocolJSONRPC, ProtocolVersion: a2a.Version,
		}},
		Version:            "1.0.0",
		DefaultInputModes:  []string{"text/plain"},
		DefaultOutputModes: []string{"text/plain"},
		Skills:             adka2a.BuildAgentSkills(deskAgent),
		Capabilities:       a2a.AgentCapabilities{Streaming: true},
	}
	devnet.Must(0, weiladk.SignAgentCard(ctx, desk, card))

	executor := devnet.Must(weiladk.NewExecutor(adka2a.ExecutorConfig{
		RunnerConfig: runner.Config{
			AppName:        deskAgent.Name(),
			Agent:          deskAgent,
			SessionService: session.InMemoryService(),
			PluginConfig:   runner.PluginConfig{Plugins: []*plugin.Plugin{deskPlugin}},
		},
	}))

	// Every request is verified (signature, body, nonce) before it reaches the
	// executor; WithAuthorize then decides who may use the desk.
	secure := devnet.Must(weiladk.SecureA2A(a2asrv.NewJSONRPCHandler(a2asrv.NewHandler(executor)), deskAgent,
		weiladk.WithAuthorize(func(ctx context.Context, caller *weiladk.Caller) error {
			// Only callers the desk's wallet listed on chain may use it.
			res, err := weiladk.Invoke(ctx, weiladk.Call{Applet: consumer, Method: "read_batch",
				Args: map[string]any{"topic": clients, "from_seq": 0, "max_count": 200}})
			if err != nil {
				return err
			}
			var entries []struct {
				Publisher string `json:"publisher"`
				Payload   struct {
					Inline string `json:"Inline"`
				} `json:"payload"`
			}
			if err := res.Decode(&entries); err != nil {
				return err
			}
			for _, e := range entries {
				var c struct{ Client string }
				if e.Publisher == desk.Address() && json.Unmarshal([]byte(e.Payload.Inline), &c) == nil && c.Client == caller.Address {
					fmt.Printf("  \033[33m[risk desk] verified caller %s… is a listed client\033[0m\n", caller.Address[:12])
					return nil
				}
			}
			fmt.Printf("  \033[31m[risk desk] verified caller %s… is NOT a client — refusing\033[0m\n", caller.Address[:12])
			return errors.New("wallet " + caller.Address + " is not a client of this desk")
		})))

	mux := http.NewServeMux()
	mux.Handle(a2asrv.WellKnownAgentCardPath, a2asrv.NewStaticAgentCardHandler(card))
	mux.Handle("/invoke", secure)
	go func() { _ = http.Serve(ln, mux) }()
	return baseURL
}

func receipt(topic string, body map[string]any) weiladk.Call {
	payload, _ := json.Marshal(body)
	return weiladk.Call{Applet: publisher, Method: "publish", Args: map[string]any{"topic": topic, "payload": string(payload)}}
}

func short(v any) string {
	s, _ := v.(string)
	if len(s) > 12 {
		return s[:12] + "…"
	}
	if s == "" {
		return "-"
	}
	return s
}
