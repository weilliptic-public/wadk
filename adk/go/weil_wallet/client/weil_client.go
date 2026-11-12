package client

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/nonce"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

type WeilClient struct {
	httpClient   *http.Client
	sentinelHost string
	creds        *S3Credentials
	signer       Signer

	wallet       *wallet.Wallet
	walletMu     sync.Mutex
	nonceTracker *nonce.NonceTracker

	// appletAddrs caches resolved applet addresses by service name. Entries
	// are pinned to the active account's home pod, so SetAccount clears it.
	appletAddrs   map[string]string
	appletAddrsMu sync.Mutex

	// submits bounds concurrent submissions; nil for no bound.
	submits chan struct{}
}

// ExecuteOptions control how an applet method call is submitted.
type ExecuteOptions struct {
	// HideArgs asks the platform not to expose the method arguments in the
	// public transaction record.
	HideArgs bool
	// NonBlocking submits without waiting for the method to run; the returned
	// TransactionResult is then empty.
	NonBlocking bool
	// Retry resends the signed call when it fails in transit (see
	// RetryPolicy). The zero value submits once.
	Retry RetryPolicy
}

// FromWalletFile creates a WeilClient by loading a wallet directly from a
// wallet.wc file.
func FromWalletFile(path string, opts ...Option) (*WeilClient, error) {
	w, err := wallet.FromWalletFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w, opts...), nil
}

// FromIdentity creates a WeilClient for an agent identity minted by the Agent
// Registry (the CLI's generate_agent_identity) — the twin of Python's
// WeilClient.from_identity.
//
// The identity is resolved to its wallet through Sentinel's
// /get_agent_wallet. That wallet is masked, so transactions are signed
// remotely through Sentinel's /sign_payload unless a custom Signer is
// configured.
func FromIdentity(ctx context.Context, identity string, opts ...Option) (*WeilClient, error) {
	cfg := newConfig(opts)
	w, err := GetAgentWallet(ctx, identity, opts...)
	if err != nil {
		return nil, err
	}
	return newWeilClient(w, cfg), nil
}

// FromAPIKey is the former name of FromIdentity.
//
// Deprecated: use FromIdentity.
func FromAPIKey(ctx context.Context, apiKey string, opts ...Option) (*WeilClient, error) {
	return FromIdentity(ctx, apiKey, opts...)
}

// NewWeilClient creates a WeilClient that signs with the given wallet.
func NewWeilClient(w *wallet.Wallet, opts ...Option) *WeilClient {
	return newWeilClient(w, newConfig(opts))
}

func newWeilClient(w *wallet.Wallet, cfg *config) *WeilClient {
	return &WeilClient{
		httpClient:   cfg.httpClient,
		sentinelHost: cfg.sentinelHost,
		creds:        cfg.creds,
		signer:       cfg.signer,
		wallet:       w,
		nonceTracker: nonce.DefaultNonceTracker(),
		appletAddrs:  map[string]string{},
		submits:      newSemaphore(cfg.concurrency),
	}
}

// acquire takes one of the client's submission slots (WithConcurrency).
func (w *WeilClient) acquire(ctx context.Context) error {
	if w.submits == nil {
		return nil
	}
	select {
	case w.submits <- struct{}{}:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

func (w *WeilClient) release() {
	if w.submits != nil {
		<-w.submits
	}
}

func newSemaphore(n int) chan struct{} {
	if n <= 0 {
		return nil
	}
	return make(chan struct{}, n)
}

// agentWalletSettle bounds how long an identity lookup is retried while
// Sentinel answers that the identity's wallet is not there yet: a lookup can
// land on a pod that has not caught up with the identity's registration.
const agentWalletSettle = 15 * time.Second

// lookupAgentWallet calls get_agent_wallet, retrying with backoff — for at
// most agentWalletSettle — a network failure, HTTP 429 or 5xx, and the
// "no persisted wallet found" a lagging pod answers. It is a read, so a
// retry is always safe; an identity that is truly unknown fails once the
// budget is spent.
func lookupAgentWallet(ctx context.Context, cfg *config, body map[string]any) ([]byte, error) {
	deadline := time.Now().Add(agentWalletSettle)
	backoff := 250 * time.Millisecond
	for {
		raw, err := api.PostJSONRaw(ctx, cfg.httpClient, cfg.sentinelHost, "get_agent_wallet", body)
		if err == nil || ctx.Err() != nil || !lookupRetryable(err) || time.Now().Add(backoff).After(deadline) {
			return raw, err
		}
		select {
		case <-time.After(backoff):
		case <-ctx.Done():
			return raw, err
		}
		backoff = min(backoff*2, 2*time.Second)
	}
}

func lookupRetryable(err error) bool {
	var httpErr *api.HTTPError
	if errors.As(err, &httpErr) && strings.Contains(httpErr.Body, "no persisted wallet found") {
		return true
	}
	return TransportFailure(err)
}

// GetAgentWallet resolves an Agent Registry identity to its masked wallet via
// Sentinel's /get_agent_wallet.
func GetAgentWallet(ctx context.Context, identity string, opts ...Option) (*wallet.Wallet, error) {
	cfg := newConfig(opts)
	body := map[string]any{
		"identity": identity,
		"unmasked": false,
	}
	if cfg.creds != nil {
		body["credentials"] = cfg.creds
	}

	raw, err := lookupAgentWallet(ctx, cfg, body)
	var httpErr *api.HTTPError
	if errors.As(err, &httpErr) {
		// Only the message: the body may carry wallet material.
		message := "no message"
		var failure map[string]any
		if json.Unmarshal([]byte(httpErr.Body), &failure) == nil {
			message = pyStr(failure["message"])
		}
		return nil, fmt.Errorf("agent wallet lookup failed: HTTP %d: %s", httpErr.StatusCode, message)
	}
	if err != nil {
		return nil, fmt.Errorf("agent wallet lookup: %w", err)
	}
	walletJSON, err := parseAgentWalletResponse(raw)
	if err != nil {
		return nil, err
	}
	w, err := wallet.FromMaskedWalletJSON(walletJSON)
	if err != nil {
		return nil, fmt.Errorf("agent wallet lookup: %w", err)
	}
	return w, nil
}

// parseAgentWalletResponse extracts the wallet file from a /get_agent_wallet
// answer — a wallet object, or a JSON string holding one — with the Python
// SDK's errors, none of which quotes the body.
func parseAgentWalletResponse(raw []byte) ([]byte, error) {
	var v any
	if err := json.Unmarshal(raw, &v); err != nil {
		return nil, fmt.Errorf("agent wallet lookup failed: non-JSON response (HTTP 200, %d bytes)", len(strings.TrimSpace(string(raw))))
	}
	switch x := v.(type) {
	case map[string]any:
		if x["type"] == "wallet" {
			return raw, nil
		}
	case string:
		if s := strings.TrimSpace(x); s != "" {
			return []byte(s), nil
		}
		return nil, errors.New("agent wallet not found for identity")
	}
	return nil, fmt.Errorf("agent wallet lookup failed: unexpected response type %s", pyTypeName(v))
}

// pyTypeName is Python's type(v).__name__ for a decoded JSON value.
func pyTypeName(v any) string {
	switch x := v.(type) {
	case nil:
		return "NoneType"
	case bool:
		return "bool"
	case float64:
		if x == float64(int64(x)) {
			return "int"
		}
		return "float"
	case string:
		return "str"
	case []any:
		return "list"
	case map[string]any:
		return "dict"
	}
	return fmt.Sprintf("%T", v)
}

// pyStr is Python's str(v) for a decoded JSON message (None for nil).
func pyStr(v any) string {
	switch x := v.(type) {
	case nil:
		return "None"
	case string:
		return x
	}
	return fmt.Sprint(v)
}

// getAppletAddressResponse matches the Sentinel response: {"Ok": "contract_id"} or {"Err": "..."}
type getAppletAddressResponse struct {
	Ok  string `json:"Ok"`
	Err string `json:"Err"`
}

// ResolveApplet resolves an applet's service name (e.g. "auditor::weil", or
// "<applet>::<domain>" registered in WNS) to its contract address, caching the
// answer for the active account.
//
// Sends the caller's own wallet_address alongside svc_name so Sentinel pins
// resolution to that wallet's home weilpod (derived server-side from the pod
// counter embedded in the address) instead of falling back to a random pod in
// its region. Without this, an applet that checks org membership can land on
// a pod whose local identity::<org> copy never saw this wallet's membership.
func (w *WeilClient) ResolveApplet(ctx context.Context, svcName string) (string, error) {
	w.appletAddrsMu.Lock()
	addr, ok := w.appletAddrs[svcName]
	w.appletAddrsMu.Unlock()
	if ok {
		return addr, nil
	}

	walletAddress := w.WalletAddress()
	body := map[string]string{
		"svc_name":       svcName,
		"wallet_address": walletAddress,
	}
	var result getAppletAddressResponse
	if err := api.PostJSON(ctx, w.httpClient, w.sentinelHost, "get_applet_address", body, &result); err != nil {
		return "", fmt.Errorf("get_applet_address failed: %w", err)
	}
	if result.Ok == "" {
		return "", fmt.Errorf("get_applet_address failed: %s", result.Err)
	}

	w.appletAddrsMu.Lock()
	// Only cache if the account did not change while resolving.
	if w.WalletAddress() == walletAddress {
		w.appletAddrs[svcName] = result.Ok
	}
	w.appletAddrsMu.Unlock()
	return result.Ok, nil
}

func (w *WeilClient) ToContractClient(contractId string) *WeilContractClient {
	return &WeilContractClient{
		httpClient: w.httpClient,
		contractId: contractId,
		client:     w,
	}
}

// Execute calls methodName on the applet at contractId.
//
// Deprecated: use ExecuteContext, which honours cancellation.
func (w *WeilClient) Execute(contractId string, methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	return w.ExecuteContext(context.Background(), contractId, methodName, methodArgs,
		ExecuteOptions{HideArgs: shouldHideArgs, NonBlocking: isNonBlocking})
}

// ExecuteContext calls methodName with JSON-encoded methodArgs on the applet
// at contractId, signed by the active account.
func (w *WeilClient) ExecuteContext(ctx context.Context, contractId string, methodName string, methodArgs string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	return w.ToContractClient(contractId).ExecuteContext(ctx, methodName, methodArgs, opts)
}

// ExecuteByName resolves svcName with ResolveApplet and calls methodName on it.
func (w *WeilClient) ExecuteByName(ctx context.Context, svcName string, methodName string, methodArgs string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	contractId, err := w.ResolveApplet(ctx, svcName)
	if err != nil {
		return nil, err
	}
	return w.ExecuteContext(ctx, contractId, methodName, methodArgs, opts)
}

// signerFor picks how account signs: the configured Signer, else locally when
// it holds a secret key, else through Sentinel for masked wallets.
func (w *WeilClient) signerFor(account *wallet.Account) (Signer, error) {
	if w.signer != nil {
		return w.signer, nil
	}
	if account.CanSign() {
		return LocalSigner{}, nil
	}

	w.walletMu.Lock()
	source := w.wallet.SourceJSON()
	w.walletMu.Unlock()
	if len(source) == 0 {
		return nil, wallet.ErrNoSecretKey
	}
	return &RemoteSigner{
		HTTPClient:   w.httpClient,
		SentinelHost: w.sentinelHost,
		Wallet:       source,
		Credentials:  w.creds,
	}, nil
}

// SignMessage signs an arbitrary JSON payload with the active account, locally
// or remotely as the wallet requires.
func (w *WeilClient) SignMessage(ctx context.Context, payload map[string]any) (string, error) {
	account := w.CurrentAccount()
	signer, err := w.signerFor(account)
	if err != nil {
		return "", err
	}
	return signer.Sign(ctx, account, payload)
}

// SetAccount switches the currently active account in the wallet.
//
// Also drops the cached applet addresses — they're pinned to whichever
// account's home pod resolved them (see ResolveApplet), so stale entries from
// the previous account must not leak into calls made under the new one.
//
// The wallet mutex is released before the applet-cache mutex is taken, so the
// two are never held together.
func (w *WeilClient) SetAccount(selected wallet.SelectedAccount) error {
	w.walletMu.Lock()
	err := w.wallet.SetIndex(selected)
	w.walletMu.Unlock()
	if err != nil {
		return err
	}

	w.appletAddrsMu.Lock()
	w.appletAddrs = map[string]string{}
	w.appletAddrsMu.Unlock()
	return nil
}

// CurrentAccount returns the active account.
func (w *WeilClient) CurrentAccount() *wallet.Account {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.CurrentAccount()
}

// DerivedAccountCount returns the number of HD-derived accounts in the wallet.
func (w *WeilClient) DerivedAccountCount() int {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.DerivedAccountCount()
}

// ExternalAccountCount returns the number of externally imported accounts in the wallet.
func (w *WeilClient) ExternalAccountCount() int {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.ExternalAccountCount()
}

// WalletAddress returns the address of the currently selected account.
func (w *WeilClient) WalletAddress() string {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.GetAddress()
}

// PublicKeyAddress returns the hex SHA-256 of the active account's
// uncompressed public key — the identity signature verifiers recover.
func (w *WeilClient) PublicKeyAddress() string {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return utils.GetAddressFromPublicKey(w.wallet.GetPublicKey())
}

// Org returns the org info if the active account has a linked organization, or nil.
func (w *WeilClient) Org() *wallet.OrgInfo {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.Org()
}

// SentinelHost returns the Sentinel base URL this client talks to.
func (w *WeilClient) SentinelHost() string {
	return w.sentinelHost
}

// HTTPClient returns the http.Client this client uses for Sentinel calls.
func (w *WeilClient) HTTPClient() *http.Client {
	return w.httpClient
}

// TxnStatus is what Sentinel's /txn_status knows of a transaction.
type TxnStatus struct {
	TxnTicket string `json:"txn_ticket"`
	// Status is "Finalized" or "Failed" once the transaction has run,
	// "InProgress" or "Confirmed" before, and "Unknown" if the pod has no
	// record of it: not run yet, or run longer ago than the signed
	// transaction TTL.
	Status    string `json:"status"`
	TxnResult string `json:"txn_result,omitempty"`
}

// TxnStatus looks up a transaction by the TxnTicket a non-blocking submit
// returned. A transaction still Unknown well after it was submitted was
// lost; resending the same signed bytes is then safe, as pods do not run a
// transaction they have already run.
func (w *WeilClient) TxnStatus(ctx context.Context, ticket string) (*TxnStatus, error) {
	var status TxnStatus
	if err := api.PostJSON(ctx, w.httpClient, w.sentinelHost, "txn_status", map[string]string{"txn_ticket": ticket}, &status); err != nil {
		return nil, err
	}
	return &status, nil
}
