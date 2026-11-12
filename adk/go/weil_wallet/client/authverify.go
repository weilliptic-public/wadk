package client

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/decred/dcrd/dcrec/secp256k1/v4/ecdsa"
)

// ErrAuth is wrapped by every *AuthError, for errors.Is.
// now is replaced in tests.
var now = time.Now

var ErrAuth = errors.New("wallet address verification failed")

// AuthError explains why auth headers were rejected. Reason is one of the
// codes the Python SDK audits: missing_auth_headers, malformed_timestamp,
// stale_timestamp, timestamp_mismatch, malformed_signed_message,
// body_hash_mismatch, unbound_body, signature_mismatch.
type AuthError struct {
	Reason string
	// Detail carries reason-specific context (missing header names, clock
	// skew, ...), suitable for audit records.
	Detail map[string]any
}

func (e *AuthError) Error() string {
	return fmt.Sprintf("%v: %s", ErrAuth, e.Reason)
}

func (e *AuthError) Unwrap() error { return ErrAuth }

func authErr(reason string, kv ...any) *AuthError {
	e := &AuthError{Reason: reason}
	if len(kv) > 0 {
		e.Detail = map[string]any{}
		for i := 0; i+1 < len(kv); i += 2 {
			e.Detail[kv[i].(string)] = kv[i+1]
		}
	}
	return e
}

// AuthCaller is the caller proven by verified auth headers.
type AuthCaller struct {
	// Address is the caller's account address (pod counter + key hash), as
	// used for on-chain identity lookups. It falls back to KeyAddress for
	// clients that do not sign their account address.
	Address string
	// KeyAddress is the 64-char address the signature recovered to.
	KeyAddress string
	// Canonical reports whether Address came from the signed message.
	Canonical bool
	// BodySHA256 and Nonce are set when the headers were bound to a body.
	BodySHA256 string
	Nonce      string
	Timestamp  time.Time
}

// VerifyAuthHeaders checks the headers built by AuthHeaders (by any Weil SDK):
// the timestamp is within maxAge (MaxAuthAge when zero), and the signature
// over X-Message recovers to the key behind X-Wallet-Address. It does not
// look at the body; use VerifyAuthRequest for body-bound headers.
func VerifyAuthHeaders(h http.Header, maxAge time.Duration) (*AuthCaller, error) {
	if maxAge <= 0 {
		maxAge = MaxAuthAge
	}
	var missing []string
	for _, name := range []string{HeaderWalletAddress, HeaderSignature, HeaderMessage, HeaderTimestamp} {
		if h.Get(name) == "" {
			missing = append(missing, strings.ToLower(name))
		}
	}
	if len(missing) > 0 {
		return nil, authErr("missing_auth_headers", "missing_headers", missing)
	}

	ts, err := strconv.ParseInt(h.Get(HeaderTimestamp), 10, 64)
	if err != nil {
		return nil, authErr("malformed_timestamp", "timestamp", h.Get(HeaderTimestamp))
	}
	skew := now().Unix() - ts
	if time.Duration(abs(skew))*time.Second > maxAge {
		return nil, authErr("stale_timestamp", "timestamp_skew_s", skew, "max_age_s", int64(maxAge/time.Second))
	}

	keyAddr := strings.ToLower(h.Get(HeaderWalletAddress))
	if len(keyAddr) < 64 {
		return nil, authErr("signature_mismatch")
	}
	keyAddr = keyAddr[len(keyAddr)-64:]
	message := h.Get(HeaderMessage)
	if !signatureRecoversTo(h.Get(HeaderSignature), []byte(message), keyAddr) {
		return nil, authErr("signature_mismatch")
	}

	var signed map[string]any
	if json.Unmarshal([]byte(message), &signed) != nil {
		return nil, authErr("malformed_signed_message")
	}
	// X-Timestamp alone is unsigned: without this check, rewriting it would
	// make any captured header set fresh again.
	if st, _ := signed["timestamp"].(string); st != h.Get(HeaderTimestamp) {
		return nil, authErr("timestamp_mismatch")
	}

	caller := &AuthCaller{Address: keyAddr, KeyAddress: keyAddr, Timestamp: time.Unix(ts, 0)}
	// The account address is trusted only when its key-hash suffix is the
	// one the signature proved: that binds the claimed pod counter to the key.
	if full, ok := signed["address"].(string); ok && full != "" && strings.HasSuffix(strings.ToLower(full), keyAddr) {
		caller.Address, caller.Canonical = full, true
	}
	caller.BodySHA256, _ = signed["body_sha256"].(string)
	caller.Nonce, _ = signed["nonce"].(string)
	return caller, nil
}

// VerifyAuthRequest is VerifyAuthHeaders plus the body binding: the signed
// message's body_sha256 must equal both X-Body-SHA256 and the hash of body
// (see SignedBody for extracting it from an HTTP request body).
func VerifyAuthRequest(h http.Header, body any, maxAge time.Duration) (*AuthCaller, error) {
	caller, err := VerifyAuthHeaders(h, maxAge)
	if err != nil {
		return nil, err
	}
	if caller.BodySHA256 == "" {
		return nil, authErr("unbound_body", "detail", "signed message omits body_sha256")
	}
	actual, err := BodySHA256(body)
	if err != nil {
		return nil, authErr("body_hash_mismatch")
	}
	if caller.BodySHA256 != actual || h.Get(HeaderBodySHA256) != actual {
		return nil, authErr("body_hash_mismatch",
			"signed_body_sha256", shortHash(caller.BodySHA256), "actual_body_sha256", shortHash(actual))
	}
	return caller, nil
}

// SignedBody extracts the part of an HTTP request body that body-bound auth
// headers cover, exactly as the Python SDK does: for a JSON-RPC request the
// A2A message (params.message, else params.request, else params); for a
// JSON object, its "message" or "request" member, else the object; non-JSON
// bodies are covered byte for byte.
func SignedBody(body []byte) any {
	var parsed any
	dec := json.NewDecoder(bytes.NewReader(body))
	dec.UseNumber()
	if err := dec.Decode(&parsed); err != nil || dec.More() {
		return body
	}
	obj, ok := parsed.(map[string]any)
	if !ok {
		return json.RawMessage(body)
	}
	pick := func(m map[string]any) any {
		for _, key := range []string{"message", "request"} {
			if v, ok := m[key].(map[string]any); ok {
				return v
			}
		}
		return nil
	}
	if params, ok := obj["params"].(map[string]any); ok {
		if v := pick(params); v != nil {
			return numberSafe(v)
		}
		return numberSafe(params)
	}
	if v := pick(obj); v != nil {
		return numberSafe(v)
	}
	return numberSafe(obj)
}

// numberSafe re-encodes a decoded value as raw JSON so json.Number literals
// survive into BodySHA256 unchanged.
func numberSafe(v any) json.RawMessage {
	raw, _ := json.Marshal(v)
	return raw
}

// signatureRecoversTo reports whether a 64-byte compact r‖s signature over
// sha256(message) recovers to a key whose address is keyAddr. Compact Weil
// signatures carry no recovery id, so both candidates are tried.
func signatureRecoversTo(signatureHex string, message []byte, keyAddr string) bool {
	sig, err := hex.DecodeString(signatureHex)
	if err != nil || len(sig) != 64 {
		return false
	}
	digest := sha256.Sum256(message)
	for recID := byte(0); recID < 2; recID++ {
		// decred's compact format: header byte 27 + recovery id
		// (+4 for compressed keys, which only changes the reported form).
		compact := append([]byte{27 + recID}, sig...)
		pub, _, err := ecdsa.RecoverCompact(compact, digest[:])
		if err != nil {
			continue
		}
		sum := sha256.Sum256(pub.SerializeUncompressed())
		if hex.EncodeToString(sum[:]) == keyAddr {
			return true
		}
	}
	return false
}

func shortHash(h string) string {
	if len(h) > 16 {
		return h[:16]
	}
	return h
}

func abs(v int64) int64 {
	if v < 0 {
		return -v
	}
	return v
}
