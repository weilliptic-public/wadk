// Package wallet provides secp256k1-backed wallet primitives for WeilChain.
//
// A wallet manages one or more accounts loaded from a wallet.wc file.
// Derived accounts have their secret keys re-derived from the stored xprv;
// external accounts carry their own secret keys.
//
// Wallets resolved from an Agent Registry API key are *masked*: Sentinel
// returns the wallet file with an obfuscated xprv, so derived accounts carry
// only their public key and address and cannot sign locally. Such wallets keep
// the raw file (see SourceJSON) so signing can be delegated to Sentinel.
//
// Use SetIndex to switch the active account at runtime.
package wallet

import (
	"bytes"
	"crypto/hmac"
	"crypto/sha256"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"os"

	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/decred/dcrd/dcrec/secp256k1/v4/ecdsa"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
)

// ErrNoSecretKey is returned when signing locally with an account that holds
// no secret key — the derived accounts of a masked (API-key) wallet. Those
// accounts must sign remotely through Sentinel.
var ErrNoSecretKey = errors.New("account has no local secret key; use remote signing")

// Account holds the secp256k1 key pair and the sentinel-minted account address.
type Account struct {
	// SecretKey is nil for masked accounts, which can only sign remotely.
	SecretKey      *secp.PrivateKey
	PublicKey      secp.PublicKey
	AccountAddress string

	// org is the organization this account acts under, or nil.
	org *OrgInfo
}

// GetAddress returns the sentinel-minted 72-char hex account address.
func (a *Account) GetAddress() string {
	return a.AccountAddress
}

// GetPublicKey returns the account's secp256k1 public key.
func (a *Account) GetPublicKey() secp.PublicKey {
	return a.PublicKey
}

// CanSign reports whether the account holds a secret key and can sign locally.
func (a *Account) CanSign() bool {
	return a.SecretKey != nil
}

// Org returns the organization this account acts under, or nil.
func (a *Account) Org() *OrgInfo {
	return a.org
}

// Sign hashes the given buffer with SHA-256 and signs it with the account's
// private key, returning the hex-encoded 64-byte compact signature (r ‖ s).
//
// The node rejects anything that is not exactly 64 bytes, so r and s are
// always written as fixed 32-byte big-endian values. Signing is RFC 6979
// deterministic and produces a low-S signature, matching libsecp256k1.
func (a *Account) Sign(buf []byte) (*string, error) {
	if a.SecretKey == nil {
		return nil, ErrNoSecretKey
	}

	digest := utils.HashSha256(buf)
	sig := ecdsa.Sign(a.SecretKey, digest)

	var compact [64]byte
	r, s := sig.R(), sig.S()
	r.PutBytesUnchecked(compact[:32])
	s.PutBytesUnchecked(compact[32:])

	hexSignature := hex.EncodeToString(compact[:])
	return &hexSignature, nil
}

// SelectedAccount identifies which account in the wallet is currently active.
type SelectedAccount struct {
	Kind  string // "derived" or "external"
	Index int
}

// Derived returns a SelectedAccount pointing to the HD-derived account at the
// given zero-based index.
func Derived(index int) SelectedAccount {
	return SelectedAccount{Kind: "derived", Index: index}
}

// External returns a SelectedAccount pointing to the external account at the
// given zero-based index.
func External(index int) SelectedAccount {
	return SelectedAccount{Kind: "external", Index: index}
}

// OrgInfo represents an organization membership linked via `wallet link-org`.
type OrgInfo struct {
	Name     string  `json:"name"`
	Subgroup *string `json:"subgroup,omitempty"`
	Purpose  string  `json:"purpose"`
}

// orgMembershipV2 is the v2 wallet file format for org membership (no purpose).
type orgMembershipV2 struct {
	Org      string `json:"org"`
	Subgroup string `json:"subgroup"`
}

// Wallet manages derived and external accounts with a currently selected account.
type Wallet struct {
	derivedAccounts []*Account
	addedAccounts   []*Account
	currentIndex    SelectedAccount

	// source is the raw wallet file, kept only for masked wallets so it can be
	// handed to Sentinel's /sign_payload for remote signing.
	source json.RawMessage
	masked bool
}

// ── wallet.wc file structs ────────────────────────────────────────────────────

type walletFile struct {
	Version          int                    `json:"version"`
	Type             string                 `json:"type"`
	Xprv             string                 `json:"xprv"`
	DerivedAccounts  []walletDerivedEntry   `json:"derived_accounts"`
	ExternalAccounts []walletExternalEntry  `json:"external_accounts"`
	SelectedAccount  *walletSelectedAccount `json:"selected_account"`
	Org              *OrgInfo               `json:"org,omitempty"`
	Orgs             []orgMembershipV2      `json:"orgs"`
	ActiveOrg        *int                   `json:"active_org,omitempty"`
}

type walletSelectedAccount struct {
	Type  string `json:"type"`
	Index int    `json:"index"`
}

type walletDerivedEntry struct {
	Index          int               `json:"index"`
	PublicKey      string            `json:"public_key"`
	AccountAddress string            `json:"account_address"`
	Orgs           []orgMembershipV2 `json:"orgs"`
	ActiveOrg      *int              `json:"active_org,omitempty"`
}

type walletExternalEntry struct {
	Index          int               `json:"index"`
	SecretKey      string            `json:"secret_key"`
	PublicKey      string            `json:"public_key"` // set in masked wallets, whose secret_key is withheld
	AccountAddress string            `json:"account_address"`
	Orgs           []orgMembershipV2 `json:"orgs"`
	ActiveOrg      *int              `json:"active_org,omitempty"`
}

// ── Constructors ──────────────────────────────────────────────────────────────

// FromWalletFile loads a Wallet from a wallet.wc file.
//
// Derived account secret keys are re-derived from the stored xprv.
// External account secret keys are read directly from the file.
// The active account is taken from the selected_account field (defaults to
// derived index 0 when absent).
func FromWalletFile(path string) (*Wallet, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("FromWalletFile: %w", err)
	}
	w, err := FromWalletJSON(data)
	if err != nil {
		return nil, fmt.Errorf("FromWalletFile: %w", err)
	}
	return w, nil
}

// FromPrivateKey builds a single-account wallet from a hex secp256k1 secret
// key and the account address Sentinel minted for it (pod counter + key
// hash) — the twin of Python's Wallet.from_private_key_and_address.
func FromPrivateKey(secretKeyHex string, accountAddress string) (*Wallet, error) {
	data, err := json.Marshal(map[string]any{
		"type": "wallet",
		"external_accounts": []map[string]any{{
			"index": 0, "secret_key": secretKeyHex, "account_address": accountAddress,
		}},
		"selected_account": map[string]any{"type": "external", "index": 0},
	})
	if err != nil {
		return nil, err
	}
	return FromWalletJSON(data)
}

// FromWalletJSON loads a Wallet from the contents of a wallet.wc file.
func FromWalletJSON(data []byte) (*Wallet, error) {
	wf, err := parseWalletFile(data)
	if err != nil {
		return nil, err
	}

	derivedAccounts, err := derivedAccountsFromXprv(wf)
	if err != nil {
		return nil, err
	}

	addedAccounts, err := externalAccounts(wf)
	if err != nil {
		return nil, err
	}

	current, err := selectedAccount(wf, len(derivedAccounts), len(addedAccounts))
	if err != nil {
		return nil, err
	}

	return &Wallet{
		derivedAccounts: derivedAccounts,
		addedAccounts:   addedAccounts,
		currentIndex:    current,
	}, nil
}

// derivedAccountsFromXprv re-derives each derived account's secret key from
// the stored xprv. A wallet holding only external accounts needs no xprv.
func derivedAccountsFromXprv(wf *walletFile) ([]*Account, error) {
	if len(wf.DerivedAccounts) == 0 {
		return nil, nil
	}

	masterKey, masterChain, err := decodeXprv(wf.Xprv)
	if err != nil {
		return nil, err
	}

	accountKey, accountChain, err := resolveAccountLevelKey(masterKey, masterChain, wf.DerivedAccounts)
	if err != nil {
		return nil, err
	}

	var derivedAccounts []*Account
	for _, entry := range wf.DerivedAccounts {
		childKey, _, err := bip32DeriveChild(accountKey, accountChain, uint32(entry.Index), false)
		if err != nil {
			return nil, fmt.Errorf("derive account %d: %w", entry.Index, err)
		}
		privKey := secp.PrivKeyFromBytes(childKey[:])
		derivedAccounts = append(derivedAccounts, &Account{
			SecretKey:      privKey,
			PublicKey:      *privKey.PubKey(),
			AccountAddress: entry.AccountAddress,
			org:            resolveOrg(wf, entry.Orgs, entry.ActiveOrg),
		})
	}
	return derivedAccounts, nil
}

// FromMaskedWalletJSON loads a masked wallet — the wallet file Sentinel's
// /get_agent_wallet returns for an Agent Registry API key.
//
// The xprv in a masked file is an obfuscated token, so derived accounts are
// built from their stored public keys and cannot sign locally. External
// accounts keep their secret keys when the file carries them. The raw file is
// retained (see SourceJSON) for remote signing.
func FromMaskedWalletJSON(data []byte) (*Wallet, error) {
	wf, err := parseWalletFile(data)
	if err != nil {
		return nil, err
	}

	var derivedAccounts []*Account
	for _, entry := range wf.DerivedAccounts {
		pkBytes, err := hex.DecodeString(entry.PublicKey)
		if err != nil {
			return nil, fmt.Errorf("derived account %d public key: %w", entry.Index, err)
		}
		pubKey, err := secp.ParsePubKey(pkBytes)
		if err != nil {
			return nil, fmt.Errorf("derived account %d public key: %w", entry.Index, err)
		}
		derivedAccounts = append(derivedAccounts, &Account{
			PublicKey:      *pubKey,
			AccountAddress: entry.AccountAddress,
			org:            resolveOrg(wf, entry.Orgs, entry.ActiveOrg),
		})
	}

	// A masked wallet carries each external account's public key, not its
	// secret (Python: Account(public_key_hex=...)).
	var addedAccounts []*Account
	for _, entry := range wf.ExternalAccounts {
		pkBytes, err := hex.DecodeString(entry.PublicKey)
		if err != nil {
			return nil, fmt.Errorf("external account %d public key: %w", entry.Index, err)
		}
		pubKey, err := secp.ParsePubKey(pkBytes)
		if err != nil {
			return nil, fmt.Errorf("external account %d public key: %w", entry.Index, err)
		}
		addedAccounts = append(addedAccounts, &Account{
			PublicKey:      *pubKey,
			AccountAddress: entry.AccountAddress,
			org:            resolveOrg(wf, entry.Orgs, entry.ActiveOrg),
		})
	}

	current, err := selectedAccount(wf, len(derivedAccounts), len(addedAccounts))
	if err != nil {
		return nil, err
	}

	return &Wallet{
		derivedAccounts: derivedAccounts,
		addedAccounts:   addedAccounts,
		currentIndex:    current,
		source:          append(json.RawMessage(nil), data...),
		masked:          true,
	}, nil
}

func parseWalletFile(data []byte) (*walletFile, error) {
	var wf walletFile
	if err := json.Unmarshal(data, &wf); err != nil {
		return nil, fmt.Errorf("parse wallet file: %w", err)
	}
	if wf.Type != "wallet" {
		return nil, fmt.Errorf("expected wallet file type 'wallet', got '%s'", wf.Type)
	}
	if len(wf.DerivedAccounts) == 0 && len(wf.ExternalAccounts) == 0 {
		return nil, fmt.Errorf("wallet file contains no accounts")
	}
	return &wf, nil
}

func externalAccounts(wf *walletFile) ([]*Account, error) {
	var accounts []*Account
	for _, entry := range wf.ExternalAccounts {
		keyBytes, err := hex.DecodeString(entry.SecretKey)
		if err != nil {
			return nil, fmt.Errorf("external account %d secret key: %w", entry.Index, err)
		}
		privKey := secp.PrivKeyFromBytes(keyBytes)
		accounts = append(accounts, &Account{
			SecretKey:      privKey,
			PublicKey:      *privKey.PubKey(),
			AccountAddress: entry.AccountAddress,
			org:            resolveOrg(wf, entry.Orgs, entry.ActiveOrg),
		})
	}
	return accounts, nil
}

// selectedAccount reads selected_account from the file, defaulting to derived
// index 0 when absent, and bounds-checks it.
func selectedAccount(wf *walletFile, derived, external int) (SelectedAccount, error) {
	kind := "derived"
	index := 0
	if wf.SelectedAccount != nil {
		kind = wf.SelectedAccount.Type
		index = wf.SelectedAccount.Index
	}

	switch kind {
	case "external":
		if index < 0 || index >= external {
			return SelectedAccount{}, fmt.Errorf(
				"selected external account index %d out of bounds (have %d)", index, external)
		}
		return External(index), nil
	default:
		if index < 0 || index >= derived {
			return SelectedAccount{}, fmt.Errorf(
				"selected derived account index %d out of bounds (have %d)", index, derived)
		}
		return Derived(index), nil
	}
}

// resolveOrg picks an account's org from its own per-account orgs[], then
// falls back to the top-level orgs[] (CLI wallet exports), then to the v1
// single org field.
func resolveOrg(wf *walletFile, accountOrgs []orgMembershipV2, accountActiveOrg *int) *OrgInfo {
	if len(accountOrgs) > 0 {
		return pickOrg(accountOrgs, accountActiveOrg)
	}
	if len(wf.Orgs) > 0 {
		return pickOrg(wf.Orgs, wf.ActiveOrg)
	}
	return wf.Org
}

func pickOrg(orgs []orgMembershipV2, active *int) *OrgInfo {
	idx := 0
	if active != nil {
		idx = *active
	}
	if idx < 0 || idx >= len(orgs) {
		return nil
	}
	m := orgs[idx]
	var subgroup *string
	if m.Subgroup != "" {
		s := m.Subgroup
		subgroup = &s
	}
	return &OrgInfo{Name: m.Org, Subgroup: subgroup}
}

// ── Account management ────────────────────────────────────────────────────────

// SetIndex switches the currently active account. Returns an error if the
// index is out of bounds for the specified kind.
func (w *Wallet) SetIndex(selected SelectedAccount) error {
	switch selected.Kind {
	case "derived":
		if selected.Index < 0 || selected.Index >= len(w.derivedAccounts) {
			return fmt.Errorf("derived account index %d out of range [0, %d)", selected.Index, len(w.derivedAccounts))
		}
	case "external":
		if selected.Index < 0 || selected.Index >= len(w.addedAccounts) {
			return fmt.Errorf("external account index %d out of range [0, %d)", selected.Index, len(w.addedAccounts))
		}
	default:
		return fmt.Errorf("unknown account kind: %s", selected.Kind)
	}
	w.currentIndex = selected
	return nil
}

// ── Accessors ─────────────────────────────────────────────────────────────────

// DerivedAccountCount returns the number of HD-derived accounts.
func (w *Wallet) DerivedAccountCount() int {
	return len(w.derivedAccounts)
}

// ExternalAccountCount returns the number of externally imported accounts.
func (w *Wallet) ExternalAccountCount() int {
	return len(w.addedAccounts)
}

// CurrentAccountIndex returns the currently selected account identifier.
func (w *Wallet) CurrentAccountIndex() SelectedAccount {
	return w.currentIndex
}

// GetAddress returns the sentinel-minted 72-char hex account address of the
// currently selected account.
func (w *Wallet) GetAddress() string {
	return w.currentAccount().GetAddress()
}

// Org returns the org the currently selected account acts under, or nil.
func (w *Wallet) Org() *OrgInfo {
	return w.currentAccount().Org()
}

// GetPublicKey returns the secp256k1 public key of the currently selected account.
func (w *Wallet) GetPublicKey() secp.PublicKey {
	return w.currentAccount().GetPublicKey()
}

// CanSign reports whether the currently selected account can sign locally.
func (w *Wallet) CanSign() bool {
	return w.currentAccount().CanSign()
}

// Masked reports whether this wallet was resolved from an Agent Registry API
// key with a masked xprv.
func (w *Wallet) Masked() bool {
	return w.masked
}

// SourceJSON returns the raw wallet file a masked wallet was built from, or
// nil for wallets loaded with their secret keys.
func (w *Wallet) SourceJSON() json.RawMessage {
	return w.source
}

// Sign hashes the buffer and signs it with the currently selected account's key.
func (w *Wallet) Sign(buf []byte) (*string, error) {
	return w.currentAccount().Sign(buf)
}

// CurrentAccount returns the currently selected account.
//
// Callers that sign should take this snapshot once and use it for both the
// transaction header and the signature, so a concurrent SetIndex cannot make
// the two disagree.
func (w *Wallet) CurrentAccount() *Account {
	return w.currentAccount()
}

func (w *Wallet) currentAccount() *Account {
	switch w.currentIndex.Kind {
	case "derived":
		return w.derivedAccounts[w.currentIndex.Index]
	default:
		return w.addedAccounts[w.currentIndex.Index]
	}
}

// ── BIP32 helpers ─────────────────────────────────────────────────────────────

var base58Alphabet = []byte("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")

// base58CheckDecode decodes a base58check string and returns the payload
// (checksum removed).
func base58CheckDecode(s string) ([]byte, error) {
	n := new(big.Int)
	for _, c := range []byte(s) {
		idx := bytes.IndexByte(base58Alphabet, c)
		if idx < 0 {
			return nil, fmt.Errorf("invalid base58 character: %c", c)
		}
		n.Mul(n, big.NewInt(58))
		n.Add(n, big.NewInt(int64(idx)))
	}

	decoded := n.Bytes()

	numLeadingZeros := 0
	for _, c := range []byte(s) {
		if c == '1' {
			numLeadingZeros++
		} else {
			break
		}
	}

	result := make([]byte, numLeadingZeros+len(decoded))
	copy(result[numLeadingZeros:], decoded)

	if len(result) < 4 {
		return nil, fmt.Errorf("base58check: payload too short")
	}
	payload := result[:len(result)-4]
	checksum := result[len(result)-4:]

	h1 := sha256.Sum256(payload)
	h2 := sha256.Sum256(h1[:])
	if !bytes.Equal(h2[:4], checksum) {
		return nil, fmt.Errorf("base58check: invalid checksum")
	}
	return payload, nil
}

// decodeXprv base58check-decodes an xprv string and returns the 32-byte
// private key and 32-byte chain code.
//
// xprv wire layout (78 bytes after checksum removal):
//
//	version(4) + depth(1) + fingerprint(4) + child_index(4)
//	+ chain_code(32) + key_prefix_0x00(1) + key(32)
func decodeXprv(xprvStr string) (key [32]byte, chain [32]byte, err error) {
	raw, err := base58CheckDecode(xprvStr)
	if err != nil {
		return key, chain, fmt.Errorf("decodeXprv: %w", err)
	}
	if len(raw) != 78 {
		return key, chain, fmt.Errorf("decodeXprv: expected 78 bytes, got %d", len(raw))
	}
	copy(chain[:], raw[13:45])
	copy(key[:], raw[46:78])
	return
}

// bip32DeriveChild derives one BIP32 child private key (hardened or normal).
func bip32DeriveChild(parentKey, parentChain [32]byte, index uint32, hardened bool) (childKey [32]byte, childChain [32]byte, err error) {
	data := make([]byte, 37)
	if hardened {
		data[0] = 0x00
		copy(data[1:33], parentKey[:])
		binary.BigEndian.PutUint32(data[33:], index+0x80000000)
	} else {
		privKey := secp.PrivKeyFromBytes(parentKey[:])
		copy(data[:33], privKey.PubKey().SerializeCompressed())
		binary.BigEndian.PutUint32(data[33:], index)
	}

	mac := hmac.New(sha512.New, parentChain[:])
	mac.Write(data)
	I := mac.Sum(nil)
	IL, IR := I[:32], I[32:]

	var ILScalar, parentScalar secp.ModNScalar
	ILScalar.SetByteSlice(IL)
	parentScalar.SetByteSlice(parentKey[:])
	ILScalar.Add(&parentScalar)

	childKeyBytes := ILScalar.Bytes()
	copy(childKey[:], childKeyBytes[:])
	copy(childChain[:], IR)
	return
}

// resolveAccountLevelKey returns the (key, chain) at the account derivation
// level. If deriving child 0 directly matches the first entry's stored
// public_key, the xprv is already at account level. Otherwise the function
// traverses m/44'/9345'/0'/0 first.
func resolveAccountLevelKey(masterKey, masterChain [32]byte, derived []walletDerivedEntry) (key [32]byte, chain [32]byte, err error) {
	if len(derived) == 0 {
		return masterKey, masterChain, nil
	}

	first := derived[0]
	childKey, _, err := bip32DeriveChild(masterKey, masterChain, uint32(first.Index), false)
	if err != nil {
		return key, chain, err
	}
	privKey := secp.PrivKeyFromBytes(childKey[:])
	pkHex := hex.EncodeToString(privKey.PubKey().SerializeCompressed())

	if pkHex == first.PublicKey {
		return masterKey, masterChain, nil
	}

	// Root xprv — traverse m/44'/9345'/0'/0.
	type step struct {
		index    uint32
		hardened bool
	}
	path := []step{{44, true}, {9345, true}, {0, true}, {0, false}}
	k, c := masterKey, masterChain
	for _, p := range path {
		k, c, err = bip32DeriveChild(k, c, p.index, p.hardened)
		if err != nil {
			return key, chain, fmt.Errorf("BIP44 path derivation: %w", err)
		}
	}
	return k, c, nil
}
