package wallet

import (
	"crypto/ecdsa"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
)

type Account struct {
	SecretKey secp.PrivateKey
	PublicKey secp.PublicKey
	Address   string
}

type Wallet struct {
	Accounts []Account
	Current  SelectedAccount
}

type SelectedAccount struct {
	Type  string
	Index int
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
	fileStr := string(fileData)
	fileStr = strings.TrimSpace(fileStr)
	fileStr = strings.ReplaceAll(fileStr, "\n", "")

	fileHexBytes, err := hex.DecodeString(fileStr)
	if err != nil {
		return nil, err
	}
	privKey := secp.PrivKeyFromBytes(fileHexBytes)

	account := Account{
		SecretKey: *privKey,
		PublicKey: *privKey.PubKey(),
		Address:   "",
	}

	return &Wallet{
		Accounts: []Account{account},
		Current:  SelectedAccount{Type: "external", Index: 0},
	}, nil
}

func (w *Wallet) GetPubcliKey() secp.PublicKey {
	return w.currentAccount().PublicKey
}

func (w *Wallet) GetAddress() string {
	return w.currentAccount().Address
}

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

type accountExportFile struct {
	Type    string `json:"type"`
	Account struct {
		SecretKey       string `json:"secret_key"`
		AccountAddress  string `json:"account_address"`
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
		Accounts: []Account{account},
		Current:  SelectedAccount{Type: "external", Index: 0},
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
	w.Accounts = append(w.Accounts, Account{
		SecretKey: *privKey,
		PublicKey: *privKey.PubKey(),
		Address:   export.Account.AccountAddress,
	})
	return nil
}

func (w *Wallet) SetIndex(sel SelectedAccount) error {
	if sel.Type != "external" {
		return fmt.Errorf("unsupported account type: %s", sel.Type)
	}
	if sel.Index < 0 || sel.Index >= len(w.Accounts) {
		return fmt.Errorf("external account index %d out of bounds (have %d external account(s))", sel.Index, len(w.Accounts))
	}
	w.Current = sel
	return nil
}

func (w *Wallet) currentAccount() Account {
	i := w.Current.Index
	if i < 0 || i >= len(w.Accounts) {
		i = 0
	}
	return w.Accounts[i]
}
