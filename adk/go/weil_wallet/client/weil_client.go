// Package client provides a high-level HTTP client for interacting with
// WeilChain smart-contract applets. It handles wallet-based signing, nonce
// management, audit-log submission, and concurrency control.
//
// Typical usage:
//
//	w, err := wallet.NewWalletFromWalletFile("wallet.wc")
//	cli := client.NewWeilClient(w)
//	result, err := cli.Execute(contractId, "methodName", `{"key":"value"}`, false, false)
package client

import (
	"bytes"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/internal/constants"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/nonce"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
	secp "github.com/decred/dcrd/dcrec/secp256k1/v4"
)

const auditAppletSvcName = "auditor"

// WeilClient is the main high-level client for WeilChain applet execution.
//
// It wraps an HTTP client (with TLS verification disabled for dev environments),
// a Wallet for signing transactions, and a NonceTracker for ordering. Methods on
// WeilClient are safe to call from multiple goroutines; internal state is protected
// by mutexes.
type WeilClient struct {
	httpClient       http.Client
	wallet           *wallet.Wallet
	walletMu         sync.Mutex
	nonceTracker     *nonce.NonceTracker
	auditContractId  string
	auditContractMu  sync.Mutex
}

// NewWeilClient creates a WeilClient from an already-constructed Wallet.
//
// The underlying HTTP transport disables TLS certificate verification, which is
// convenient for development environments running self-signed certificates.
// The default request timeout is 5 seconds.
func NewWeilClient(wallet *wallet.Wallet) *WeilClient {
	transport := &http.Transport{
		TLSClientConfig: &tls.Config{
			InsecureSkipVerify: true,
		},
	}

	client := http.Client{
		Transport: transport,
		Timeout:   5 * time.Second,
	}

	return &WeilClient{
		httpClient:      client,
		wallet:          wallet,
		nonceTracker:    nonce.DefaultNonceTracker(),
		auditContractId: "",
	}
}

// NewWeilClientFromAccountExportFile constructs a WeilClient by loading a
// single-account export file (account.wc). This is a convenience wrapper around
// wallet.NewWalletFromAccountExportFile followed by NewWeilClient.
func NewWeilClientFromAccountExportFile(path string) (*WeilClient, error) {
	w, err := wallet.NewWalletFromAccountExportFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w), nil
}

// NewWeilClientFromWalletFile constructs a WeilClient by loading a multi-account
// wallet file (wallet.wc). Derived accounts are re-derived from the stored xprv;
// external accounts are read directly from the file.
func NewWeilClientFromWalletFile(path string) (*WeilClient, error) {
	w, err := wallet.NewWalletFromWalletFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w), nil
}

// AddAccountFromExportFile appends an additional account from a sentinel account
// export file. The active account does not change. Safe for concurrent use.
func (w *WeilClient) AddAccountFromExportFile(path string) error {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.AddAccountFromExportFile(path)
}

// SetAccount switches the active signing account. sel identifies either a
// derived or external account by type and index. Returns an error if the index
// is out of bounds for the requested account list.
func (w *WeilClient) SetAccount(sel wallet.SelectedAccount) error {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.SetIndex(sel)
}

func (w *WeilClient) activePublicKey() secp.PublicKey {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.GetPubcliKey()
}

func (w *WeilClient) activeAddress() string {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.GetAddress()
}

func (w *WeilClient) sign(buf []byte) (*string, error) {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.Sign(buf)
}

// getAppletAddressResponse matches the Sentinel response: {"Ok": "contract_id"} or {"Err": "..."}
type getAppletAddressResponse struct {
	Ok  string `json:"Ok"`
	Err string `json:"Err"`
}

// getAuditContractId resolves and caches the audit applet contract address from the Sentinel API.
func (w *WeilClient) getAuditContractId() (string, error) {
	w.auditContractMu.Lock()
	defer w.auditContractMu.Unlock()
	if w.auditContractId != "" {
		return w.auditContractId, nil
	}
	url := fmt.Sprintf("https://%s/get_applet_address", constants.SENTINEL_HOST)
	body := map[string]string{"svc_name": auditAppletSvcName}
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return "", fmt.Errorf("get_applet_address request: %w", err)
	}
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("get_applet_address request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := w.httpClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("get_applet_address: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return "", fmt.Errorf("get_applet_address failed: HTTP %d", resp.StatusCode)
	}
	var result getAppletAddressResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", fmt.Errorf("get_applet_address decode: %w", err)
	}
	if result.Ok == "" {
		return "", fmt.Errorf("get_applet_address failed: %s", result.Err)
	}
	w.auditContractId = result.Ok
	return w.auditContractId, nil
}

// ToContractClient returns a WeilContractClient bound to the given contractId.
// All Execute calls on the returned client will target that specific contract.
func (w *WeilClient) ToContractClient(contractId string) *WeilContractClient {
	return &WeilContractClient{
		httpClient: &w.httpClient,
		contractId: contractId,
		client:     w,
	}
}

// Execute calls a method on the specified contract and returns the transaction result.
//
//   - contractId: the target applet's on-chain address.
//   - methodName: the exported method to invoke.
//   - methodArgs: JSON-encoded argument payload.
//   - shouldHideArgs: when true the arguments are encrypted before submission.
//   - isNonBlocking: when true the platform responds immediately without waiting
//     for the transaction to be finalized.
func (w *WeilClient) Execute(contractId string, methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	txnResult, err := w.ToContractClient(contractId).Execute(methodName, methodArgs, shouldHideArgs, isNonBlocking)

	if err != nil {
		return nil, err
	}

	return txnResult, nil
}

// Audit submits a log message to the auditor applet, creating a verifiable
// on-chain audit trail entry. The audit applet contract address is resolved
// from the Sentinel API on the first call and cached for subsequent calls.
//
// The log is submitted as a non-blocking transaction so the call returns
// quickly without waiting for finalization.
func (w *WeilClient) Audit(log string) error {
	contractId, err := w.getAuditContractId()
	if err != nil {
		return err
	}

	type Arg struct {
		Log string `json:"log"`
	}

	args := Arg{
		Log: log,
	}

	argsBytes, err := json.Marshal(args)

	if err != nil {
		return err
	}

	_, err = w.Execute(contractId, "audit", string(argsBytes), false, true)

	if err != nil {
		return err
	}

	return nil
}
