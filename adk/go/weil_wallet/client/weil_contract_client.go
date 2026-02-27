package client

import (
	"encoding/hex"
	"net/http"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/contract"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/utils"
)

type WeilContractClient struct {
	httpClient *http.Client
	contractId string
	client     *WeilClient
}

type ExecuteArgs struct {
	ContractAddress    string
	ContractMethod     string
	ContractInputBytes *types.Option[string]
	ShouldHideArgs     bool
}

type NonceFailureResponse struct {
	ExpectedNonce uint32 `json:"expected_nonce"`
	ReceivedNonce uint32 `json:"received_nonce"`
	Message       string `json:"message"`
	Status        string `json:"status"`
}

func (w *WeilContractClient) Execute(methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	publicKey := w.client.wallet.GetPubcliKey()
	fromAddr := utils.GetAddressFromPublicKey(publicKey)
	toAddr := fromAddr
	contractId := w.contractId
	weilpodCounter, err := contract.PodCounter(contractId)

	if err != nil {
		return nil, err
	}

	publicKeyHex := hex.EncodeToString(publicKey.SerializeUncompressed())

	args := &ExecuteArgs{
		ContractAddress:    w.contractId,
		ContractMethod:     methodName,
		ContractInputBytes: types.NewSomeOption(&methodArgs),
		ShouldHideArgs:     shouldHideArgs,
	}

	nonce := int(time.Now().UnixMilli())
	txnHeader := transaction.NewTransactionHeader(nonce, publicKeyHex, fromAddr, toAddr, weilpodCounter)

	signature, err := w.SignExecuteArgs(txnHeader, args)

	if err != nil {
		return nil, err
	}

	txnHeader.SetSignature(*signature)
	baseTxn := transaction.NewBaseTransaction(txnHeader)

	response, err := w.SubmitSignedArgs(*signature, baseTxn, args, isNonBlocking)

	if err != nil {
		return nil, err
	}

	return response, nil
}

func (w WeilContractClient) SignExecuteArgs(txnHeader *transaction.TransactionHeader, args *ExecuteArgs) (*string, error) {
	jsonPayload := map[string]interface{}{
		"nonce":     txnHeader.Nonce,
		"from_addr": txnHeader.FromAddr,
		"to_addr":   txnHeader.ToAddr,
		"user_txn": map[string]any{
			"type":                 "SmartContractExecutor",
			"contract_address":     args.ContractAddress,
			"contract_method":      args.ContractMethod,
			"contract_input_bytes": args.ContractInputBytes,
			"should_hide_args":     args.ShouldHideArgs,
		},
	}

	jsonPayloadBtreemap := transaction.ValueToBtreeMap(jsonPayload)
	jsonPayloadJson, err := transaction.BtreeMapToJson(jsonPayloadBtreemap)

	if err != nil {
		return nil, err
	}

	signature, err := w.client.wallet.Sign(jsonPayloadJson)

	if err != nil {
		return nil, err
	}

	return signature, nil
}

func (w WeilContractClient) SubmitSignedArgs(signature string, txn *transaction.BaseTransaction, args *ExecuteArgs, isNonBlocking bool) (*transaction.TransactionResult, error) {

	typeStr := "SmartContractExecutor"
	payload := api.SubmitTxnRequest{
		Transaction: &api.Transaction{
			IsXpod: false,
			TxnHeader: transaction.NewTransactionHeaderWithSignature(
				txn.Header.Nonce,
				txn.Header.PublicKey,
				txn.Header.FromAddr,
				txn.Header.ToAddr,
				signature,
				txn.Header.WeilpodCounter,
			),
			Verifier: &api.Verifier{
				Ty: "DefaultVerifier",
			},
			UserTxn: &api.UserTransaction{
				Ty:                 typeStr,
				ContractAddress:    args.ContractAddress,
				ContractMethod:     args.ContractMethod,
				ContractInputBytes: args.ContractInputBytes,
				ShouldHideArgs:     args.ShouldHideArgs,
			},
		},
	}

	response, err := api.SubmitTransaction(w.httpClient, payload, isNonBlocking)

	if err != nil {
		return nil, err
	}

	return response, nil
}
