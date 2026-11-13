package weiladk

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

// Identity is an agent's on-chain identity: a wallet and a client that signs
// and submits applet calls with it.
type Identity struct {
	client *client.WeilClient
}

// NewIdentity wraps an existing weil_wallet client.
func NewIdentity(c *client.WeilClient) *Identity {
	return &Identity{client: c}
}

// FromWalletFile loads an identity from a wallet.wc file.
func FromWalletFile(path string, opts ...client.Option) (*Identity, error) {
	c, err := client.FromWalletFile(path, opts...)
	if err != nil {
		return nil, err
	}
	return NewIdentity(c), nil
}

// FromIdentity resolves an Agent Registry identity (minted with the CLI's
// generate_agent_identity) to the agent's on-chain identity — the twin of
// Python's @weil_ai.agent(identity=...). Its wallet is masked, so calls are
// signed remotely by Sentinel unless a custom signer is configured with
// client.WithSigner.
func FromIdentity(ctx context.Context, identity string, opts ...client.Option) (*Identity, error) {
	c, err := client.FromIdentity(ctx, identity, opts...)
	if err != nil {
		return nil, err
	}
	return NewIdentity(c), nil
}

// FromAPIKey is the former name of FromIdentity.
//
// Deprecated: use FromIdentity.
func FromAPIKey(ctx context.Context, apiKey string, opts ...client.Option) (*Identity, error) {
	return FromIdentity(ctx, apiKey, opts...)
}

// Client returns the underlying weil_wallet client.
func (i *Identity) Client() *client.WeilClient {
	return i.client
}

// Address returns the active account's address.
func (i *Identity) Address() string {
	return i.client.WalletAddress()
}

// Org returns the org the active account acts under, or nil.
func (i *Identity) Org() *wallet.OrgInfo {
	return i.client.Org()
}

// AuthHeaders returns fresh wallet-signed headers for MCP servers and other
// services that verify Weil signatures.
func (i *Identity) AuthHeaders(ctx context.Context) (map[string]string, error) {
	return i.client.AuthHeaders(ctx)
}

// HTTPClient returns an http.Client that signs every request with this
// identity's wallet — for MCP toolsets talking to Weil-secured MCP servers
// (Go or Python), or hand-built A2A clients (A2AClientFactory).
func (i *Identity) HTTPClient() *http.Client {
	return i.client.AuthHTTPClient()
}

// Invoke signs and submits call as this identity and, unless the call is
// FireAndForget, waits for the applet method to run.
//
// A method that returns an error on chain yields a *transaction.AppletError,
// so callers can tell applet rejections apart from transport failures.
func (i *Identity) Invoke(ctx context.Context, call Call) (*Result, error) {
	if call.Method == "" {
		return nil, errors.New("weiladk: call has no method")
	}
	contractID, err := resolveWithRetry(ctx, call, i.client)
	if err != nil {
		return nil, err
	}
	args, err := call.encodeArgs()
	if err != nil {
		return nil, fmt.Errorf("weiladk: encode args for %s.%s: %w", call.Applet, call.Method, err)
	}

	txn, err := i.client.ExecuteContext(ctx, contractID, call.Method, args, client.ExecuteOptions{
		HideArgs:    call.HideArgs,
		NonBlocking: call.FireAndForget,
		Retry:       call.Retry,
	})
	if err != nil {
		return nil, fmt.Errorf("weiladk: %s.%s: %w", call.Applet, call.Method, err)
	}

	result := &Result{Call: call, ContractID: contractID, Txn: txn}
	if !call.FireAndForget {
		// Surface an applet-level Err without forcing callers to decode.
		var appletErr *transaction.AppletError
		if err := txn.Decode(nil); errors.As(err, &appletErr) {
			return result, appletErr
		}
	}
	return result, nil
}

// Call is one applet method invocation.
type Call struct {
	Applet Applet
	Method string
	// Args is JSON-encoded as the method's argument object. A string,
	// []byte or json.RawMessage is taken as already-encoded JSON. Nil sends
	// an empty object.
	Args any
	// HideArgs keeps the arguments out of the public transaction record.
	HideArgs bool
	// FireAndForget submits without waiting for the platform to run the
	// method; the Result then carries no output. This is the platform's
	// non-blocking submission and is independent of an Action's Mode.
	FireAndForget bool
	// Retry resends the signed call when it fails in transit, keeping its
	// transaction ID (see client.RetryPolicy). Zero submits once. Audit
	// entries use client.DurableRetry so none is lost under load.
	Retry client.RetryPolicy
}

func (c Call) encodeArgs() (string, error) {
	switch v := c.Args.(type) {
	case nil:
		return "{}", nil
	case string:
		return v, nil
	case []byte:
		return string(v), nil
	case json.RawMessage:
		return string(v), nil
	default:
		b, err := json.Marshal(v)
		if err != nil {
			return "", err
		}
		return string(b), nil
	}
}

// Result is the outcome of an Invoke.
type Result struct {
	Call       Call
	ContractID string
	Txn        *transaction.TransactionResult
}

// Decode unwraps the applet method's return value into out.
func (r *Result) Decode(out any) error {
	return r.Txn.Decode(out)
}

// resolveWithRetry resolves call's applet, retrying under call.Retry while
// Sentinel cannot be reached — a cold client's first audit entry must not be
// lost to a congested get_applet_address — for at most SignedTxnTTL.
func resolveWithRetry(ctx context.Context, call Call, c *client.WeilClient) (string, error) {
	if call.Retry.MaxAttempts <= 1 {
		return call.Applet.Resolve(ctx, c)
	}
	ctx, cancel := context.WithTimeout(ctx, client.SignedTxnTTL)
	defer cancel()
	backoff, maxBackoff := call.Retry.InitialBackoff, call.Retry.MaxBackoff
	if backoff <= 0 {
		backoff = 250 * time.Millisecond
	}
	if maxBackoff <= 0 {
		maxBackoff = 10 * time.Second
	}
	for attempt := 1; ; attempt++ {
		id, err := call.Applet.Resolve(ctx, c)
		if err == nil || attempt >= call.Retry.MaxAttempts || ctx.Err() != nil || !client.TransportFailure(err) {
			return id, err
		}
		select {
		case <-time.After(backoff):
		case <-ctx.Done():
			return id, err
		}
		backoff = min(backoff*2, maxBackoff)
	}
}
