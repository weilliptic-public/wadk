package client

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/contract"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// Service names of systemic applets.
const (
	PublisherApplet = "publisher::weil"
	PensieveApplet  = "pensieve::weil"
	// IdentityAppletPrefix + org names an org's identity applet, e.g.
	// "identity::weil".
	IdentityAppletPrefix = "identity::"
)

// Sentinel talks to Sentinel without a wallet: applet resolution and
// unsigned, read-only applet calls — what a server needs to check a caller's
// permissions without an identity of its own.
type Sentinel struct {
	host       string
	httpClient *http.Client
}

// NewSentinel returns a Sentinel client configured by the usual options
// (WithSentinelHost, WithInsecureSkipVerify, WithHTTPClient, ...).
func NewSentinel(opts ...Option) *Sentinel {
	cfg := newConfig(opts)
	return &Sentinel{host: cfg.sentinelHost, httpClient: cfg.httpClient}
}

// Sentinel returns a wallet-less Sentinel client sharing w's host and HTTP
// client.
func (w *WeilClient) Sentinel() *Sentinel {
	return &Sentinel{host: w.sentinelHost, httpClient: w.httpClient}
}

// Host returns the Sentinel base URL.
func (s *Sentinel) Host() string { return s.host }

// ResolveApplet resolves an applet's service name to its contract address.
// walletAddress, when set, pins resolution to that wallet's home pod (see
// WeilClient.ResolveApplet).
func (s *Sentinel) ResolveApplet(ctx context.Context, svcName, walletAddress string) (string, error) {
	body := map[string]string{"svc_name": svcName}
	if walletAddress != "" {
		body["wallet_address"] = walletAddress
	}
	var result getAppletAddressResponse
	if err := api.PostJSON(ctx, s.httpClient, s.host, "get_applet_address", body, &result); err != nil {
		return "", fmt.Errorf("resolve applet %q: %w", svcName, err)
	}
	if result.Ok == "" {
		return "", fmt.Errorf("resolve applet %q: %s", svcName, result.Err)
	}
	return result.Ok, nil
}

// Query calls a read-only applet method without signing: the transaction
// carries no public key and no signature, and from names the wallet the
// question is about rather than one proving anything. The chain runs
// queries that change no state without authorization; this is how the
// secured checks ask key_has_purpose. args is JSON-encoded unless it is
// already a string.
func (s *Sentinel) Query(ctx context.Context, contractID, method string, args any, from string) (*transaction.TransactionResult, error) {
	var argsJSON string
	switch v := args.(type) {
	case string:
		argsJSON = v
	default:
		b, err := json.Marshal(v)
		if err != nil {
			return nil, err
		}
		argsJSON = string(b)
	}
	podCounter, err := contract.PodCounter(contractID)
	if err != nil {
		return nil, err
	}
	header := transaction.NewTransactionHeader(int(time.Now().UnixMilli()), "", from, from, podCounter, nil)
	payload := api.SubmitTxnRequest{Transaction: &api.Transaction{
		TxnHeader: transaction.NewTransactionHeaderWithSignature(header.Nonce, "", from, from, "",
			podCounter, header.Salt, nil),
		Verifier: &api.Verifier{Ty: "DefaultVerifier"},
		UserTxn: &api.UserTransaction{
			Ty:                 "SmartContractExecutor",
			ContractAddress:    contractID,
			ContractMethod:     method,
			ContractInputBytes: types.NewSomeOption(&argsJSON),
			ShouldHideArgs:     true,
		},
	}}
	return api.SubmitTransaction(ctx, s.httpClient, s.host, payload, false)
}

// CreateTopic creates a WeilStream topic on the publisher applet.
func (w *WeilClient) CreateTopic(ctx context.Context, topic, description string, retentionBytes int64) (*transaction.TransactionResult, error) {
	args, _ := PythonDumpsValue(struct {
		Topic            string `json:"topic"`
		Description      string `json:"description"`
		RetentionInBytes int64  `json:"retention_in_bytes"`
	}{topic, description, retentionBytes}, false)
	return w.ExecuteByName(ctx, PublisherApplet, "create_topic", string(args), ExecuteOptions{})
}

// Publish publishes payload to a WeilStream topic through the platform
// publisher applet (PublisherApplet).
func (w *WeilClient) Publish(ctx context.Context, topic, payload string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	return w.PublishTo(ctx, PublisherApplet, topic, payload, opts)
}

// PublishTo publishes payload to topic on the publisher applet svcName, e.g.
// an org's "publisher::<org>" — Python's publish(topic, payload,
// svc_name=...).
func (w *WeilClient) PublishTo(ctx context.Context, svcName, topic, payload string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	args, _ := PythonDumpsValue(struct {
		Topic   string `json:"topic"`
		Payload string `json:"payload"`
	}{topic, payload}, false)
	return w.ExecuteByName(ctx, svcName, "publish", string(args), opts)
}
