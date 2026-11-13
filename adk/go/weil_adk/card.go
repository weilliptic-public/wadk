package weiladk

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"slices"

	"github.com/a2aproject/a2a-go/v2/a2a"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// Signed agent cards are an optional extra on top of the Python-compatible
// protocol: a server can prove its own wallet in its card, and a client can
// pin it (WithExpectedWallet). Python agents serve and accept plain cards.

const (
	// IdentityExtensionURI marks the agent-card extension that carries a
	// WeilChain identity.
	IdentityExtensionURI = "https://weilliptic.ai/a2a/ext/identity/v1"
)

var (
	// ErrNoCardIdentity is returned for an agent card without a verifiable
	// identity.
	ErrNoCardIdentity = errors.New("weiladk: agent card carries no verifiable identity")
)

// CardIdentity is the identity an agent card advertises.
type CardIdentity struct {
	Wallet    string   `json:"wallet"`
	PublicKey string   `json:"public_key"`
	Org       string   `json:"org,omitempty"`
	URLs      []string `json:"urls"`
	Signature string   `json:"signature"`
}

func (c CardIdentity) signedPayload(name string) map[string]any {
	return map[string]any{"agent": name, "org": c.Org, "public_key": c.PublicKey, "urls": c.URLs, "wallet": c.Wallet}
}

// SignAgentCard adds id's identity to an A2A agent card you built the ADK
// way: a card extension with the wallet, its public key, and a signature over
// the card's name, wallet and endpoint URLs. Call it after SupportedInterfaces
// is final.
func SignAgentCard(ctx context.Context, id *Identity, card *a2a.AgentCard) error {
	ci := CardIdentity{Wallet: id.Address(), PublicKey: id.client.PublicKeyHex()}
	if org := id.Org(); org != nil {
		ci.Org = org.Name
	}
	for _, iface := range card.SupportedInterfaces {
		if iface != nil {
			ci.URLs = append(ci.URLs, iface.URL)
		}
	}
	if len(ci.URLs) == 0 {
		return errors.New("weiladk: sign agent card: card has no SupportedInterfaces")
	}
	sig, err := id.client.SignMessage(ctx, ci.signedPayload(card.Name))
	if err != nil {
		return fmt.Errorf("weiladk: sign agent card: %w", err)
	}
	ci.Signature = sig

	var params map[string]any
	raw, _ := json.Marshal(ci)
	_ = json.Unmarshal(raw, &params)
	card.Capabilities.Extensions = slices.DeleteFunc(card.Capabilities.Extensions,
		func(e a2a.AgentExtension) bool { return e.URI == IdentityExtensionURI })
	card.Capabilities.Extensions = append(card.Capabilities.Extensions, a2a.AgentExtension{
		URI:         IdentityExtensionURI,
		Description: "WeilChain wallet identity of this agent",
		Params:      params,
	})
	return nil
}

// VerifyAgentCard checks the identity a card advertises: the signature, that
// the wallet address belongs to the signing key, and that it covers this
// card's name and every endpoint URL.
func VerifyAgentCard(card *a2a.AgentCard) (*CardIdentity, error) {
	if card == nil {
		return nil, ErrNoCardIdentity
	}
	for _, ext := range card.Capabilities.Extensions {
		if ext.URI != IdentityExtensionURI {
			continue
		}
		var ci CardIdentity
		raw, _ := json.Marshal(ext.Params)
		if err := json.Unmarshal(raw, &ci); err != nil {
			return nil, fmt.Errorf("%w: %v", ErrNoCardIdentity, err)
		}
		if err := client.VerifySignature(ci.PublicKey, ci.Signature, ci.signedPayload(card.Name)); err != nil {
			return nil, fmt.Errorf("%w: %v", ErrNoCardIdentity, err)
		}
		if !client.AccountAddressMatches(ci.Wallet, ci.PublicKey) {
			return nil, fmt.Errorf("%w: wallet %s does not belong to the signing key", ErrNoCardIdentity, ci.Wallet)
		}
		for _, iface := range card.SupportedInterfaces {
			if iface != nil && !slices.Contains(ci.URLs, iface.URL) {
				return nil, fmt.Errorf("%w: endpoint %s is not covered by the signed identity", ErrNoCardIdentity, iface.URL)
			}
		}
		return &ci, nil
	}
	return nil, ErrNoCardIdentity
}

func hasIdentityExtension(card *a2a.AgentCard) bool {
	if card == nil {
		return false
	}
	return slices.ContainsFunc(card.Capabilities.Extensions, func(e a2a.AgentExtension) bool { return e.URI == IdentityExtensionURI })
}
