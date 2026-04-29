// Package wallet provides secp256k1-backed wallet primitives for WeilChain.
//
// A wallet manages one or more accounts loaded from a wallet.wc file.
// Derived accounts have their secret keys re-derived from the stored xprv;
// external accounts carry their own secret keys.
//
// Use SetIndex to switch the active account at runtime.
package wallet

import (
	"bytes"
	"crypto/ecdsa"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"strings"

	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
)

// Account holds the secp256k1 key pair and the sentinel-minted account address.
type Account struct {
	SecretKey secp.PrivateKey
	PublicKey secp.PublicKey
	Address   string
}

// SelectedAccount identifies which account in the wallet is currently active.
type SelectedAccount struct {
	Type  string // "derived" or "external"
	Index int
}

// Derived returns a SelectedAccount pointing to the derived account at index.
func Derived(index int) SelectedAccount { return SelectedAccount{Type: "derived", Index: index} }

// External returns a SelectedAccount pointing to the external account at index.
func External(index int) SelectedAccount { return SelectedAccount{Type: "external", Index: index} }

// Wallet manages derived and external accounts with a currently selected account.
type Wallet struct {
	derivedAccounts []Account
	addedAccounts   []Account
	currentIndex    SelectedAccount
}

func readTheFile(fileName string) ([]byte, error) {
	fileData, err := os.ReadFile(fileName)
	if err != nil {
		return nil, err
	}
	return fileData, nil
}

// argument: absolute path to the file containing private key as hex string
func NewWallet(absPath string) (*Wallet, error) {
	fileData, err := readTheFile(absPath)
	if err != nil {
		return nil, err
	}
	fileStr := strings.TrimSpace(string(fileData))
	fileStr = strings.ReplaceAll(fileStr, "\n", "")

	fileHexBytes, err := hex.DecodeString(fileStr)
	if err != nil {
		return nil, err
	}
	privKey := secp.PrivKeyFromBytes(fileHexBytes)
	account := Account{SecretKey: *privKey, PublicKey: *privKey.PubKey(), Address: ""}
	return &Wallet{
		derivedAccounts: nil,
		addedAccounts:   []Account{account},
		currentIndex:    External(0),
	}, nil
}

func (w *Wallet) GetPubcliKey() secp.PublicKey { // kept for API compatibility
	return w.currentAccount().PublicKey
}

func (w *Wallet) GetAddress() string { return w.currentAccount().Address }

func (w *Wallet) Sign(buf []byte) (*string, error) {
	digest := utils.HashSha256(buf)
	acc := w.currentAccount()
	r, s, err := ecdsa.Sign(rand.Reader, (&acc.SecretKey).ToECDSA(), digest)
	if err != nil {
		return nil, err
	}
	signature := append(r.Bytes(), s.Bytes()...)
	hexSignature := hex.EncodeToString(signature)
	return &hexSignature, nil
}

// ── account.wc (single-account export) ────────────────────────────────────────

type accountExportFile struct {
	Type    string `json:"type"`
	Account struct {
		SecretKey      string `json:"secret_key"`
		AccountAddress string `json:"account_address"`
	} `json:"account"`
}

func NewWalletFromAccountExportFile(path string) (*Wallet, error) {
	raw, err := readTheFile(path)
	if err != nil {
		return nil, err
	}
	var export accountExportFile
	if err := json.Unmarshal(raw, &export); err != nil {
		return nil, err
	}
	if export.Type != "account" {
		return nil, fmt.Errorf("expected export type 'account', got '%s'", export.Type)
	}
	keyHex := strings.TrimSpace(export.Account.SecretKey)
	keyHex = strings.ReplaceAll(keyHex, "\n", "")
	keyBytes, err := hex.DecodeString(keyHex)
	if err != nil {
		return nil, err
	}
	privKey := secp.PrivKeyFromBytes(keyBytes)
	account := Account{
		SecretKey: *privKey,
		PublicKey: *privKey.PubKey(),
		Address:   export.Account.AccountAddress,
	}
	return &Wallet{
		derivedAccounts: nil,
		addedAccounts:   []Account{account},
		currentIndex:    External(0),
	}, nil
}

func (w *Wallet) AddAccountFromExportFile(path string) error {
	raw, err := readTheFile(path)
	if err != nil {
		return err
	}
	var export accountExportFile
	if err := json.Unmarshal(raw, &export); err != nil {
		return err
	}
	if export.Type != "account" {
		return fmt.Errorf("expected export type 'account', got '%s'", export.Type)
	}
	keyHex := strings.TrimSpace(export.Account.SecretKey)
	keyHex = strings.ReplaceAll(keyHex, "\n", "")
	keyBytes, err := hex.DecodeString(keyHex)
	if err != nil {
		return err
	}
	privKey := secp.PrivKeyFromBytes(keyBytes)
	w.addedAccounts = append(w.addedAccounts, Account{
		SecretKey: *privKey,
		PublicKey: *privKey.PubKey(),
		Address:   export.Account.AccountAddress,
	})
	return nil
}

// ── wallet.wc (multi-account) ────────────────────────────────────────────────

type walletFile struct {
	Version          int `json:"version"`
	Type             string `json:"type"`
	Xprv             string `json:"xprv"`
	DerivedAccounts  []walletDerivedEntry `json:"derived_accounts"`
	ExternalAccounts []walletExternalEntry `json:"external_accounts"`
	SelectedAccount  *walletSelectedAccount `json:"selected_account"`
}

type walletSelectedAccount struct {
	Type  string `json:"type"`
	Index int    `json:"index"`
}

type walletDerivedEntry struct {
	Index          int    `json:"index"`
	PublicKey      string `json:"public_key"`
	AccountAddress string `json:"account_address"`
}

type walletExternalEntry struct {
	Index          int    `json:"index"`
	SecretKey      string `json:"secret_key"`
	AccountAddress string `json:"account_address"`
}

// NewWalletFromWalletFile loads a Wallet from a wallet.wc file.
func NewWalletFromWalletFile(path string) (*Wallet, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("NewWalletFromWalletFile: %w", err)
	}
	var wf walletFile
	if err := json.Unmarshal(data, &wf); err != nil {
		return nil, fmt.Errorf("NewWalletFromWalletFile: parse: %w", err)
	}
	if wf.Type != "wallet" {
		return nil, fmt.Errorf("NewWalletFromWalletFile: expected type 'wallet', got '%s'", wf.Type)
	}
	if len(wf.DerivedAccounts) == 0 && len(wf.ExternalAccounts) == 0 {
		return nil, fmt.Errorf("NewWalletFromWalletFile: wallet file contains no accounts")
	}

	masterKey, masterChain, err := decodeXprv(wf.Xprv)
	if err != nil {
		return nil, fmt.Errorf("NewWalletFromWalletFile: %w", err)
	}
	accountKey, accountChain, err := resolveAccountLevelKey(masterKey, masterChain, wf.DerivedAccounts)
	if err != nil {
		return nil, fmt.Errorf("NewWalletFromWalletFile: %w", err)
	}

	var derivedAccounts []Account
	for _, entry := range wf.DerivedAccounts {
		childKey, _, err := bip32DeriveChild(accountKey, accountChain, uint32(entry.Index), false)
		if err != nil {
			return nil, fmt.Errorf("NewWalletFromWalletFile: derive account %d: %w", entry.Index, err)
		}
		privKey := secp.PrivKeyFromBytes(childKey[:])
		derivedAccounts = append(derivedAccounts, Account{
			SecretKey: *privKey,
			PublicKey: *privKey.PubKey(),
			Address:   entry.AccountAddress,
		})
	}

	var addedAccounts []Account
	for _, entry := range wf.ExternalAccounts {
		keyBytes, err := hex.DecodeString(entry.SecretKey)
		if err != nil {
			return nil, fmt.Errorf("NewWalletFromWalletFile: external account %d secret key: %w", entry.Index, err)
		}
		privKey := secp.PrivKeyFromBytes(keyBytes)
		addedAccounts = append(addedAccounts, Account{
			SecretKey: *privKey,
			PublicKey: *privKey.PubKey(),
			Address:   entry.AccountAddress,
		})
	}

	kind := "derived"
	index := 0
	if wf.SelectedAccount != nil {
		kind = wf.SelectedAccount.Type
		index = wf.SelectedAccount.Index
	}

	var current SelectedAccount
	switch kind {
	case "external":
		if index >= len(addedAccounts) {
			return nil, fmt.Errorf("NewWalletFromWalletFile: selected external account index %d out of bounds (have %d)", index, len(addedAccounts))
		}
		current = External(index)
	default:
		if index >= len(derivedAccounts) {
			return nil, fmt.Errorf("NewWalletFromWalletFile: selected derived account index %d out of bounds (have %d)", index, len(derivedAccounts))
		}
		current = Derived(index)
	}

	return &Wallet{derivedAccounts: derivedAccounts, addedAccounts: addedAccounts, currentIndex: current}, nil
}

func (w *Wallet) SetIndex(sel SelectedAccount) error {
	switch sel.Type {
	case "derived":
		if sel.Index < 0 || sel.Index >= len(w.derivedAccounts) {
			return fmt.Errorf("derived account index %d out of bounds (have %d derived account(s))", sel.Index, len(w.derivedAccounts))
		}
	case "external":
		if sel.Index < 0 || sel.Index >= len(w.addedAccounts) {
			return fmt.Errorf("external account index %d out of bounds (have %d external account(s))", sel.Index, len(w.addedAccounts))
		}
	default:
		return fmt.Errorf("unknown account type: %s", sel.Type)
	}
	w.currentIndex = sel
	return nil
}

func (w *Wallet) currentAccount() Account {
	switch w.currentIndex.Type {
	case "derived":
		return w.derivedAccounts[w.currentIndex.Index]
	default:
		return w.addedAccounts[w.currentIndex.Index]
	}
}

// ── BIP32 helpers (for wallet.wc xprv) ────────────────────────────────────────

var base58Alphabet = []byte("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")

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

// decodeXprv base58check-decodes an xprv string and returns the 32-byte private key and 32-byte chain code.
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
