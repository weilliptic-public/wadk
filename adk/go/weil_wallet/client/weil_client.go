package client

import (
	"crypto/tls"
	"net/http"
	"time"

	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/nonce"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/transaction"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/wallet"
)

type WeilClient struct {
	httpClient   http.Client
	wallet       *wallet.Wallet
	nonceTracker *nonce.NonceTracker
}

func NewWeilClient(wallet *wallet.Wallet) *WeilClient {
	transport := &http.Transport{
		TLSClientConfig: &tls.Config{
			InsecureSkipVerify: true,
		},
	}

	client := http.Client{
		Transport: transport,
		Timeout: 5 * time.Second,
	}

	return &WeilClient{
		httpClient:   client,
		wallet:       wallet,
		nonceTracker: nonce.DefaultNonceTracker(),
	}
}

func (w *WeilClient) ToContractClient(contractId string) *WeilContractClient {
	return &WeilContractClient{
		httpClient: &w.httpClient,
		contractId: contractId,
		client:     w,
	}
}

func (w *WeilClient) Execute(contractId string, methodName string, methodArgs string) (*transaction.TransactionResult, error) {
	txnResult, err := w.ToContractClient(contractId).Execute(methodName, methodArgs)

	if err != nil {
		return nil, err
	}

	return txnResult, nil
}
