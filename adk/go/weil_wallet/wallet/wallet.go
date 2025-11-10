package wallet

import (
	"crypto/ecdsa"
	"crypto/rand"
	"encoding/hex"
	"os"
	"strings"

	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/utils"
)

type Account struct {
	SecretKey secp.PrivateKey
	PublicKey secp.PublicKey
}

type Wallet struct {
	Account Account
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
	}

	return &Wallet{Account: account}, nil
}

func (w *Wallet) GetPubcliKey() secp.PublicKey {
	return w.Account.PublicKey
}

func (w *Wallet) Sign(buf []byte) (*string, error) {
	digest := utils.HashSha256(buf)

	r, s, err := ecdsa.Sign(rand.Reader, w.Account.SecretKey.ToECDSA(), digest)
	if err != nil {
		return nil, err
	}
	signature := append(r.Bytes(), s.Bytes()...)

	hexSignature := hex.EncodeToString(signature)
	return &hexSignature, nil
}
