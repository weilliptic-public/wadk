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

type WeilClient struct {
	httpClient       http.Client
	wallet           *wallet.Wallet
	walletMu         sync.Mutex
	nonceTracker     *nonce.NonceTracker
	auditContractId  string
	auditContractMu  sync.Mutex
}

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

func NewWeilClientFromAccountExportFile(path string) (*WeilClient, error) {
	w, err := wallet.NewWalletFromAccountExportFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w), nil
}

func NewWeilClientFromWalletFile(path string) (*WeilClient, error) {
	w, err := wallet.NewWalletFromWalletFile(path)
	if err != nil {
		return nil, err
	}
	return NewWeilClient(w), nil
}

func (w *WeilClient) AddAccountFromExportFile(path string) error {
	w.walletMu.Lock()
	defer w.walletMu.Unlock()
	return w.wallet.AddAccountFromExportFile(path)
}

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

func (w *WeilClient) ToContractClient(contractId string) *WeilContractClient {
	return &WeilContractClient{
		httpClient: &w.httpClient,
		contractId: contractId,
		client:     w,
	}
}

func (w *WeilClient) Execute(contractId string, methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	txnResult, err := w.ToContractClient(contractId).Execute(methodName, methodArgs, shouldHideArgs, isNonBlocking)

	if err != nil {
		return nil, err
	}

	return txnResult, nil
}

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
