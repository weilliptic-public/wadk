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

const auditAppletSvcName = "auditor::weil"

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

// FromWalletFile creates a WeilClient by loading a wallet directly from a
// wallet.wc file.
func FromWalletFile(path string) (*WeilClient, error) {
	w, err := wallet.FromWalletFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w), nil
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

// getAppletAddressResponse matches the Sentinel response: {"Ok": "contract_id"} or {"Err": "..."}
type getAppletAddressResponse struct {
	Ok  string `json:"Ok"`
	Err string `json:"Err"`
}

// getAuditContractId resolves and caches the audit applet contract address from the Sentinel API.
//
// Sends the caller's own wallet_address alongside svc_name so Sentinel pins
// resolution to that wallet's home weilpod (derived server-side from the pod
// counter embedded in the address) instead of falling back to a random pod in
// its region. Without this, validate_and_persist_receipt can land on a pod
// whose local identity::<org> copy never saw this wallet's membership.
func (w *WeilClient) getAuditContractId() (string, error) {
	w.auditContractMu.Lock()
	defer w.auditContractMu.Unlock()
	if w.auditContractId != "" {
		return w.auditContractId, nil
	}
	url := fmt.Sprintf("%s/get_applet_address", constants.SENTINEL_HOST)
	body := map[string]string{
		"svc_name":       auditAppletSvcName,
		"wallet_address": w.WalletAddress(),
	}
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
		Log string  `json:"log"`
		Org *string `json:"org,omitempty"`
	}

	w.walletMu.Lock()
	var orgName *string
	if orgInfo := w.wallet.Org(); orgInfo != nil {
		s := orgInfo.Name
		orgName = &s
	}
	w.walletMu.Unlock()

	args := Arg{
		Log: log,
		Org: orgName,
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

// GetReceiptForCommit reads back the receipt content currently persisted for
// commitHash, or nil if nothing has been persisted for it yet.
//
// Used by the same-turn-commit-gap amend path to fetch the payload an agent-run
// mid-turn commit already shipped — with empty prompts/usage, since that commit
// landed before Stop had computed them — so it can be merged and re-persisted
// under the same commit hash.
func (w *WeilClient) GetReceiptForCommit(commitHash string) (*string, error) {
	contractId, err := w.getAuditContractId()
	if err != nil {
		return nil, err
	}

	args := struct {
		CommitHash string `json:"commit_hash"`
	}{CommitHash: commitHash}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	resp, err := w.Execute(contractId, "get_receipt_for_commit", string(argsBytes), false, false)
	if err != nil {
		return nil, err
	}

	return parseGetReceiptForCommitResult(resp.TxnResult)
}

// parseGetReceiptForCommitResult decodes the txn_result of a
// get_receipt_for_commit call into the receipt content, or nil when nothing is
// persisted for the commit.
//
// The value is double-wrapped: the platform puts every contract call's result
// in an {"Ok": ...}/{"Err": ...} envelope, and "Ok"'s value is itself the
// callee's return value *re-serialized to a JSON string* rather than embedded
// directly. So this needs two decode passes: unwrap the envelope, then parse
// the resulting string to reach the actual optional receipt.
func parseGetReceiptForCommitResult(txnResult string) (*string, error) {
	var envelope any
	if err := json.Unmarshal([]byte(txnResult), &envelope); err != nil {
		return nil, fmt.Errorf("failed to parse get_receipt_for_commit response: %w", err)
	}

	okValue := envelope
	if obj, isObj := envelope.(map[string]any); isObj {
		if errValue, hasErr := obj["Err"]; hasErr {
			return nil, fmt.Errorf("get_receipt_for_commit returned an error: %v", errValue)
		}
		if value, hasOk := obj["Ok"]; hasOk {
			okValue = value
		}
	}

	inner := okValue
	switch value := okValue.(type) {
	case nil:
		return nil, nil
	case string:
		if err := json.Unmarshal([]byte(value), &inner); err != nil {
			return nil, fmt.Errorf("failed to parse get_receipt_for_commit inner value: %w", err)
		}
	}

	switch value := inner.(type) {
	case nil:
		return nil, nil
	case string:
		return &value, nil
	default:
		return nil, fmt.Errorf("unexpected get_receipt_for_commit payload shape: %v", value)
	}
}

// SetAccount switches the currently active account in the wallet.
//
// Also drops the cached audit contract id — it's pinned to whichever account's
// home pod resolved it (see getAuditContractId), so a stale entry from the
// previous account must not leak into calls made under the new one.
//
// The wallet mutex is released before the audit-contract mutex is taken:
// getAuditContractId holds them in the opposite order, so holding both here
// would risk a deadlock.
func (w *WeilClient) SetAccount(selected wallet.SelectedAccount) error {
	w.walletMu.Lock()
	err := w.wallet.SetIndex(selected)
	w.walletMu.Unlock()
	if err != nil {
		return err
	}

	w.auditContractMu.Lock()
	w.auditContractId = ""
	w.auditContractMu.Unlock()
	return nil
}

// DerivedAccountCount returns the number of HD-derived accounts in the wallet.
func (w *WeilClient) DerivedAccountCount() int {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.DerivedAccountCount()
}

// ExternalAccountCount returns the number of externally imported accounts in the wallet.
func (w *WeilClient) ExternalAccountCount() int {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.ExternalAccountCount()
}

// WalletAddress returns the address of the currently selected account.
func (w *WeilClient) WalletAddress() string {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.GetAddress()
}

// Org returns the org info if the wallet has a linked organization, or nil.
func (w *WeilClient) Org() *wallet.OrgInfo {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.Org()
}
