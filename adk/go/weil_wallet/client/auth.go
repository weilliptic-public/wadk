package client

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"net/http"
	"strconv"
	"time"
)

// Auth header names, shared with the Python SDK (weil_ai.auth) so Go and
// Python agents and servers verify each other.
const (
	HeaderWalletAddress = "X-Wallet-Address"
	HeaderSignature     = "X-Signature"
	HeaderMessage       = "X-Message"
	HeaderTimestamp     = "X-Timestamp"
	// HeaderBodySHA256 and HeaderNonce are sent only by body-bound headers
	// (AuthHeadersForBody).
	HeaderBodySHA256 = "X-Body-SHA256"
	HeaderNonce      = "X-Nonce"
)

// MaxAuthAge is how old signed auth headers may be before servers reject
// them, as in the Python SDK.
const MaxAuthAge = 5 * time.Minute

// AuthHeaders returns wallet-signed headers proving the caller controls the
// active account, for MCP servers, A2A agents and other HTTP services that
// verify Weil signatures (weil_ai's weil_middleware, secured.Middleware).
//
// The signed message (X-Message) is the canonical JSON
// {"address": <account address>, "timestamp": "<unix seconds>"}. The account
// address carries the pod counter, which the key alone cannot prove, so it
// rides inside the signature; X-Wallet-Address is the key-derived address
// the signature recovers to. Servers reject headers older than MaxAuthAge,
// so build them per request.
func (w *WeilClient) AuthHeaders(ctx context.Context) (map[string]string, error) {
	return w.authHeaders(ctx, nil)
}

// AuthHeadersForBody is AuthHeaders bound to a request body: the signature
// also covers the body's hash (see BodySHA256) and a fresh nonce, so the
// body cannot be swapped and the headers cannot be reused for another
// request. Weil A2A servers require it.
func (w *WeilClient) AuthHeadersForBody(ctx context.Context, body any) (map[string]string, error) {
	if body == nil {
		body = map[string]any{}
	}
	return w.authHeaders(ctx, body)
}

func (w *WeilClient) authHeaders(ctx context.Context, body any) (map[string]string, error) {
	timestamp := strconv.FormatInt(time.Now().Unix(), 10)
	payload := map[string]any{
		"address":   w.WalletAddress(),
		"timestamp": timestamp,
	}
	if body != nil {
		sum, err := BodySHA256(body)
		if err != nil {
			return nil, fmt.Errorf("hash request body: %w", err)
		}
		nonce := make([]byte, 16)
		if _, err := rand.Read(nonce); err != nil {
			return nil, err
		}
		payload["body_sha256"] = sum
		payload["nonce"] = hex.EncodeToString(nonce)
	}

	message, err := CanonicalJSON(payload)
	if err != nil {
		return nil, err
	}
	signature, err := w.SignMessage(ctx, payload)
	if err != nil {
		return nil, err
	}

	headers := map[string]string{
		HeaderWalletAddress: w.PublicKeyAddress(),
		HeaderSignature:     signature,
		HeaderMessage:       string(message),
		HeaderTimestamp:     timestamp,
	}
	if body != nil {
		headers[HeaderBodySHA256] = payload["body_sha256"].(string)
		headers[HeaderNonce] = payload["nonce"].(string)
	}
	return headers, nil
}

// SetAuthHeaders signs req for its body (read by the caller and passed in,
// nil for none) and sets the auth headers on it.
func (w *WeilClient) SetAuthHeaders(req *http.Request, body []byte) error {
	var (
		headers map[string]string
		err     error
	)
	if body == nil {
		headers, err = w.AuthHeaders(req.Context())
	} else {
		headers, err = w.AuthHeadersForBody(req.Context(), SignedBody(body))
	}
	if err != nil {
		return err
	}
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	return nil
}
