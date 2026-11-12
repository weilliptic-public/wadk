package client

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strings"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

// Signer produces the hex signature for a payload on behalf of an account.
//
// payload is the JSON object to sign. Signers that sign locally must sign its
// canonical form (see CanonicalJSON): SHA-256 over key-sorted, compact JSON,
// which is what the node rebuilds to verify.
type Signer interface {
	Sign(ctx context.Context, account *wallet.Account, payload map[string]any) (string, error)
}

// SignerFunc adapts a function to the Signer interface.
type SignerFunc func(ctx context.Context, account *wallet.Account, payload map[string]any) (string, error)

func (f SignerFunc) Sign(ctx context.Context, account *wallet.Account, payload map[string]any) (string, error) {
	return f(ctx, account, payload)
}

// CanonicalJSON returns the bytes a payload is signed over: compact JSON with
// object keys sorted at every level, matching the node's BTreeMap encoding.
func CanonicalJSON(payload map[string]any) ([]byte, error) {
	return transaction.BtreeMapToJson(transaction.ValueToBtreeMap(payload))
}

// LocalSigner signs with the account's own secret key.
type LocalSigner struct{}

func (LocalSigner) Sign(_ context.Context, account *wallet.Account, payload map[string]any) (string, error) {
	canonical, err := CanonicalJSON(payload)
	if err != nil {
		return "", err
	}
	signature, err := account.Sign(canonical)
	if err != nil {
		return "", err
	}
	return *signature, nil
}

// RemoteSigner delegates signing to Sentinel's /sign_payload endpoint. It is
// what masked (API-key) wallets use, since they hold no local secret key.
type RemoteSigner struct {
	HTTPClient   *http.Client
	SentinelHost string
	// Wallet is the masked wallet file Sentinel unmasks to sign.
	Wallet      json.RawMessage
	Credentials *S3Credentials
}

func (s *RemoteSigner) Sign(ctx context.Context, _ *wallet.Account, payload map[string]any) (string, error) {
	if len(s.Wallet) == 0 {
		return "", errors.New("remote signing requires a masked (API-key) wallet")
	}

	body := map[string]any{"payload": payload, "wallet": s.Wallet}
	if s.Credentials != nil { // sent only when set, as the Python SDK does
		body["credentials"] = s.Credentials
	}
	raw, err := api.PostJSONRaw(ctx, s.HTTPClient, s.SentinelHost, "sign_payload", body)
	if err != nil {
		return "", err
	}
	return parseSignPayloadResponse(raw)
}

// parseSignPayloadResponse accepts {"signature": hex}, {"Ok": hex}, or a bare
// hex body, and surfaces {"Err": ...} as an error.
func parseSignPayloadResponse(raw []byte) (string, error) {
	var obj map[string]json.RawMessage
	if err := json.Unmarshal(raw, &obj); err == nil {
		for _, key := range []string{"signature", "Ok"} {
			var sig string
			if v, ok := obj[key]; ok && json.Unmarshal(v, &sig) == nil && sig != "" {
				return sig, nil
			}
		}
		if v, ok := obj["Err"]; ok {
			return "", fmt.Errorf("sign_payload failed: %s", string(v))
		}
		return "", fmt.Errorf("sign_payload failed: response carries no signature")
	}

	bare := strings.Trim(strings.TrimSpace(string(raw)), `"`)
	if decoded, err := hex.DecodeString(bare); err == nil && len(decoded) == 64 {
		return bare, nil
	}
	return "", fmt.Errorf("sign_payload failed: unexpected response")
}
