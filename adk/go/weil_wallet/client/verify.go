package client

import (
	"encoding/hex"
	"errors"
	"fmt"
	"strings"

	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/decred/dcrd/dcrec/secp256k1/v4/ecdsa"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
)

// ErrBadSignature is returned when a signature does not verify.
var ErrBadSignature = errors.New("signature does not verify")

// VerifySignature checks a hex 64-byte compact signature (as produced by
// SignMessage) over the canonical JSON of payload, against a hex-encoded
// secp256k1 public key (compressed or uncompressed).
func VerifySignature(publicKeyHex string, signatureHex string, payload map[string]any) error {
	pub, err := parsePublicKey(publicKeyHex)
	if err != nil {
		return err
	}
	sig, err := hex.DecodeString(signatureHex)
	if err != nil || len(sig) != 64 {
		return fmt.Errorf("%w: malformed signature", ErrBadSignature)
	}
	var r, s secp.ModNScalar
	if r.SetByteSlice(sig[:32]) || s.SetByteSlice(sig[32:]) {
		return fmt.Errorf("%w: signature overflows the group order", ErrBadSignature)
	}

	canonical, err := CanonicalJSON(payload)
	if err != nil {
		return err
	}
	if !ecdsa.NewSignature(&r, &s).Verify(utils.HashSha256(canonical), pub) {
		return ErrBadSignature
	}
	return nil
}

// AccountAddressMatches reports whether a sentinel-minted account address
// belongs to the public key: an address is a 4-byte pod counter followed by
// SHA-256 of the uncompressed public key, the same binding the node checks.
func AccountAddressMatches(accountAddress string, publicKeyHex string) bool {
	pub, err := parsePublicKey(publicKeyHex)
	if err != nil {
		return false
	}
	addr, err := hex.DecodeString(accountAddress)
	if err != nil || len(addr) != 36 {
		return false
	}
	return hex.EncodeToString(addr[4:]) == strings.ToLower(utils.GetAddressFromPublicKey(*pub))
}

// PublicKeyHex returns the active account's uncompressed public key, hex
// encoded — what verifiers need alongside a signature.
func (w *WeilClient) PublicKeyHex() string {
	pub := w.CurrentAccount().GetPublicKey()
	return hex.EncodeToString(pub.SerializeUncompressed())
}

func parsePublicKey(publicKeyHex string) (*secp.PublicKey, error) {
	raw, err := hex.DecodeString(publicKeyHex)
	if err != nil {
		return nil, fmt.Errorf("malformed public key: %w", err)
	}
	pub, err := secp.ParsePubKey(raw)
	if err != nil {
		return nil, fmt.Errorf("malformed public key: %w", err)
	}
	return pub, nil
}
