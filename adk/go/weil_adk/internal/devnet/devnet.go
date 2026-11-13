// Package devnet wires the examples to a local WeilChain platform: per-agent
// wallets provisioned from Sentinel, Claude as the model, and a console view
// of what the agents do.
//
// Environment:
//
//	WEIL_SENTINEL_HOST  Sentinel base URL (default DefaultSentinelHost, the
//	                    dev platform)
//	WEIL_TLS_INSECURE=1 skip verifying Sentinel's TLS certificate. Skipped
//	                    anyway for a local platform (an IP address,
//	                    localhost or sentinel-local...), whose certificate
//	                    is self-signed, unless WEIL_TLS_VERIFY=1
//	API_KEY             Anthropic API key (falls back to ANTHROPIC_API_KEY)
//	CLAUDE_MODEL        model ID (default claude-opus-5-5)
//
// Each agent's identity (see Agent), by the agent's name, e.g. roll-desk:
//
//	WEIL_IDENTITY_ROLL_DESK  an Agent Registry identity (weil_...)
//	WEIL_WALLET_ROLL_DESK    a wallet.wc file
//	                         (neither: ~/.weilliptic/agents/roll-desk.wc,
//	                         created on first use)
package devnet

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/runner"
	"google.golang.org/adk/v2/session"
	"google.golang.org/genai"

	weiladk "github.com/weilliptic-public/wadk/adk/go/weil_adk"
	"github.com/weilliptic-public/wadk/adk/go/weil_adk/claudemodel"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// Agent returns the identity the agent called name acts under, one of:
//
//   - WEIL_IDENTITY_<NAME>: an Agent Registry identity (weil_...), whose
//     wallet Sentinel holds and signs with;
//   - WEIL_WALLET_<NAME>: a wallet.wc file;
//   - otherwise ~/.weilliptic/agents/<name>.wc, created on first use.
//
// <NAME> is name upper-cased, with every character other than a letter or
// digit turned into "_" (roll-desk → ROLL_DESK). Every agent has an identity
// of its own: two different names resolving to the same wallet is an error,
// so a misconfiguration cannot make one agent act as another.
func Agent(ctx context.Context, name string) (*weiladk.Identity, error) {
	return resolve(ctx, name, ClientOptions(), true)
}

// AgentOn is Agent for a program that may talk to any platform, such as a
// production one: its Sentinel and TLS settings come from opts, not from
// the local-platform defaults, and no wallet is ever created — the agent
// needs its WEIL_IDENTITY_<NAME> or WEIL_WALLET_<NAME>.
func AgentOn(ctx context.Context, name string, opts ...client.Option) (*weiladk.Identity, error) {
	return resolve(ctx, name, opts, false)
}

func resolve(ctx context.Context, name string, opts []client.Option, create bool) (*weiladk.Identity, error) {
	key := EnvKey(name)
	var (
		id     *weiladk.Identity
		err    error
		source string
	)
	switch {
	case os.Getenv("WEIL_IDENTITY_"+key) != "":
		source = "WEIL_IDENTITY_" + key
		id, err = weiladk.FromIdentity(ctx, os.Getenv(source), opts...)
	case os.Getenv("WEIL_WALLET_"+key) != "":
		source = "WEIL_WALLET_" + key
		id, err = weiladk.FromWalletFile(os.Getenv(source), opts...)
	case create:
		source = "~/.weilliptic/agents/" + name + ".wc"
		id, err = AgentWallet(ctx, name)
	default:
		return nil, fmt.Errorf("agent %s has no identity: set WEIL_IDENTITY_%s (an Agent Registry identity) or WEIL_WALLET_%s (a wallet file)", name, key, key)
	}
	if err != nil {
		return nil, fmt.Errorf("identity of agent %s (%s): %w", name, source, err)
	}

	claimedMu.Lock()
	defer claimedMu.Unlock()
	if other, ok := claimed[id.Address()]; ok && other != name {
		return nil, fmt.Errorf("agents %s and %s both act as wallet %s: give %s its own WEIL_IDENTITY_%s or WEIL_WALLET_%s",
			other, name, id.Address(), name, key, key)
	}
	if _, ok := claimed[id.Address()]; !ok {
		fmt.Fprintf(os.Stderr, "agent %-18s wallet %s (%s)\n", name, id.Address(), source)
	}
	claimed[id.Address()] = name
	return id, nil
}

// EnvKey is name as Agent spells it in environment variables.
func EnvKey(name string) string {
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

var (
	claimedMu sync.Mutex
	claimed   = map[string]string{} // wallet address → agent name
)

// DefaultSentinelHost is the Sentinel the examples use when
// WEIL_SENTINEL_HOST is not set: the dev platform.
const DefaultSentinelHost = "https://sentinel.weilliptic.ai"

// SentinelHost is WEIL_SENTINEL_HOST, or DefaultSentinelHost.
func SentinelHost() string {
	if host := os.Getenv("WEIL_SENTINEL_HOST"); host != "" {
		return host
	}
	return DefaultSentinelHost
}

// ClientOptions points weil_wallet clients at SentinelHost. Its TLS
// certificate is verified (see VerifyTLS).
func ClientOptions() []client.Option {
	host := SentinelHost()
	opts := []client.Option{client.WithSentinelHost(host)}
	if !VerifyTLS(host) {
		opts = append(opts, client.WithInsecureSkipVerify())
	}
	return opts
}

// VerifyTLS reports whether to verify host's TLS certificate: always, except
// for a local platform, whose certificate is self-signed, or with
// WEIL_TLS_INSECURE=1. WEIL_TLS_VERIFY=1 verifies a local one too.
func VerifyTLS(host string) bool {
	switch {
	case os.Getenv("WEIL_TLS_VERIFY") == "1":
		return true
	case os.Getenv("WEIL_TLS_INSECURE") == "1":
		return false
	}
	return !IsLocalHost(host)
}

// IsLocalHost reports whether host (a URL or a bare host) names a local
// platform: an IP address, localhost, or a sentinel-local... name.
func IsLocalHost(host string) bool {
	h := host
	if u, err := url.Parse(host); err == nil && u.Host != "" {
		h = u.Hostname()
	}
	return net.ParseIP(h) != nil || h == "localhost" || strings.HasPrefix(h, "sentinel-local")
}

// Claude returns the model the examples run on: claudemodel.NewModel, the
// twin of gemini.NewModel. API_KEY overrides ANTHROPIC_API_KEY.
func Claude() model.LLM {
	return Must(claudemodel.NewModel(context.Background(), os.Getenv("CLAUDE_MODEL"),
		&claudemodel.Config{APIKey: os.Getenv("API_KEY")}))
}

// E2EModel is for end-to-end test runs of the examples: with
// WEIL_E2E_MODEL=claude it returns Claude (see Claude) in place of the model
// an example builds — OpenAI or Gemini, whose keys a test platform may not
// have. Otherwise it returns the example's own model and error unchanged.
func E2EModel[M model.LLM](m M, err error) (model.LLM, error) {
	if os.Getenv("WEIL_E2E_MODEL") == "claude" {
		return Claude(), nil
	}
	if err != nil {
		return nil, err
	}
	return m, nil
}

// AgentWallet returns the identity for name, creating its wallet through
// Sentinel's /create_wallet on first use and keeping it in
// ~/.weilliptic/agents/<name>.wc. These are development wallets for a local
// platform; the file holds the wallet's xprv.
//
// Prefer Agent, which also honours an agent's WEIL_IDENTITY_/WEIL_WALLET_.
func AgentWallet(ctx context.Context, name string) (*weiladk.Identity, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, err
	}
	dir := filepath.Join(home, ".weilliptic", "agents")
	path := filepath.Join(dir, name+".wc")
	if _, err := os.Stat(path); err == nil {
		return weiladk.FromWalletFile(path, ClientOptions()...)
	}

	// /create_wallet is unauthenticated, so any identity's client will do
	// for the HTTP settings; build a throwaway config the same way.
	probe := client.NewWeilClient(nil, ClientOptions()...)
	var resp struct {
		Ok *struct {
			Xprv       string `json:"xprv"`
			HomeRegion string `json:"home_region"`
			Account    struct {
				Index          int    `json:"index"`
				PublicKey      string `json:"public_key"`
				AccountAddress string `json:"account_address"`
			} `json:"account"`
		} `json:"Ok"`
		Err string `json:"Err"`
	}
	if err := api.PostJSON(ctx, probe.HTTPClient(), probe.SentinelHost(), "create_wallet",
		map[string]string{"region": "asia-south1"}, &resp); err != nil {
		return nil, fmt.Errorf("create wallet for %s: %w", name, err)
	}
	if resp.Ok == nil {
		return nil, fmt.Errorf("create wallet for %s: %s", name, resp.Err)
	}

	wallet := map[string]any{
		"version":          2,
		"type":             "wallet",
		"xprv":             resp.Ok.Xprv,
		"home_region":      resp.Ok.HomeRegion,
		"selected_account": map[string]any{"type": "derived", "index": 0},
		"derived_accounts": []map[string]any{{
			"index":           resp.Ok.Account.Index,
			"public_key":      resp.Ok.Account.PublicKey,
			"account_address": resp.Ok.Account.AccountAddress,
			"orgs":            []any{},
		}},
		"external_accounts": []any{},
	}
	data, err := json.MarshalIndent(wallet, "", "  ")
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return nil, err
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		return nil, err
	}
	return weiladk.FromWalletFile(path, ClientOptions()...)
}

// Run sends prompt to the runner and prints the agents' tool calls, tool
// results and replies as they happen. It returns the final reply text.
func Run(ctx context.Context, r *runner.Runner, sessionID, prompt string) (string, error) {
	fmt.Printf("\n\033[1m▶ user:\033[0m %s\n", prompt)
	var final string
	for ev, err := range r.Run(ctx, "operator", sessionID, genai.NewContentFromText(prompt, genai.RoleUser), agent.RunConfig{}) {
		if err != nil {
			return final, err
		}
		printEvent(ev)
		if ev.IsFinalResponse() && ev.Content != nil {
			if text := textOf(ev.Content); text != "" {
				final = text
			}
		}
	}
	return final, nil
}

func printEvent(ev *session.Event) {
	// Failures such as a remote A2A agent refusing the call arrive as events
	// with an ErrorMessage and no content, not as an error from Run.
	if ev.ErrorMessage != "" {
		fmt.Printf("  \033[31m[%s] ✗ %s\033[0m\n", ev.Author, clip(ev.ErrorMessage, 300))
	}
	if ev.Content == nil {
		return
	}
	for _, p := range ev.Content.Parts {
		switch {
		case p.FunctionCall != nil:
			args, _ := json.Marshal(p.FunctionCall.Args)
			fmt.Printf("  \033[36m[%s] → %s\033[0m %s\n", ev.Author, p.FunctionCall.Name, clip(string(args), 200))
		case p.FunctionResponse != nil:
			resp, _ := json.Marshal(p.FunctionResponse.Response)
			color := "\033[32m"
			if _, failed := p.FunctionResponse.Response["error"]; failed {
				color = "\033[31m"
			}
			fmt.Printf("  %s[%s] ← %s\033[0m %s\n", color, ev.Author, p.FunctionResponse.Name, clip(string(resp), 300))
		case p.Text != "" && !p.Thought:
			fmt.Printf("\033[1m◀ %s:\033[0m %s\n", ev.Author, strings.TrimSpace(p.Text))
		}
	}
}

func textOf(c *genai.Content) string {
	var b strings.Builder
	for _, p := range c.Parts {
		if !p.Thought {
			b.WriteString(p.Text)
		}
	}
	return strings.TrimSpace(b.String())
}

func clip(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "…"
}

// Section prints a heading.
func Section(title string) {
	fmt.Printf("\n\033[1;35m━━ %s ━━\033[0m\n", title)
}

// SetupCall makes an example's own setup call (create_topic, read_batch, ...)
// as id, riding out a busy local platform: the signed call is resent on
// transport failures and on the platform's own timeout (client.DurableRetry);
// a read is repeated for up to 30s while the topic it names is not yet
// visible (UNKNOWN_TOPIC: it was just created, on another pod); and a resent
// create_topic answered "already exists" counts as done.
func SetupCall(ctx context.Context, id *weiladk.Identity, call weiladk.Call) (*weiladk.Result, error) {
	call.Retry = client.DurableRetry
	deadline := time.Now().Add(30 * time.Second)
	backoff := 500 * time.Millisecond
	for {
		res, err := id.Invoke(ctx, call)
		var appletErr *transaction.AppletError
		switch {
		case err == nil:
			return res, nil
		case errors.As(err, &appletErr) && call.Method == "create_topic" && strings.Contains(appletErr.Message, "already exists"):
			return res, nil
		case errors.As(err, &appletErr) && strings.Contains(appletErr.Message, "UNKNOWN_TOPIC") && time.Now().Add(backoff).Before(deadline):
			time.Sleep(backoff)
			backoff = min(backoff*2, 4*time.Second)
		default:
			return res, err
		}
	}
}

// Must panics on error; the examples are scripts.
func Must[T any](v T, err error) T {
	if err != nil {
		panic(err)
	}
	return v
}

// NewRunner builds a runner with an in-memory session service.
func NewRunner(app string, root agent.Agent, plugins runner.PluginConfig) *runner.Runner {
	return Must(runner.New(runner.Config{
		AppName:           app,
		Agent:             root,
		SessionService:    session.InMemoryService(),
		PluginConfig:      plugins,
		AutoCreateSession: true,
	}))
}
