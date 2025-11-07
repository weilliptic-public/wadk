package client

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/api"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/contract"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/internal/constants"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/transaction"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/utils"
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
}

type NonceFailureResponse struct {
	ExpectedNonce uint32 `json:"expected_nonce"`
	ReceivedNonce uint32 `json:"received_nonce"`
	Message       string `json:"message"`
	Status        string `json:"status"`
}

func (w *WeilContractClient) Execute(methodName string, methodArgs string) (*transaction.TransactionResult, error) {
	publicKey := w.client.wallet.GetPubcliKey()
	fromAddr := utils.GetAddressFromPublicKey(publicKey)
	toAddr := fromAddr
	contractId := w.contractId
	weilpodCounter, err := contract.PodCounter(contractId)

	if err != nil {
		return nil, err
	}

	nonceKey := fmt.Sprintf("%v%s%s", weilpodCounter, "$", fromAddr)
	publicKeyHex := hex.EncodeToString(publicKey.SerializeUncompressed())

	args := ExecuteArgs{
		ContractAddress:    w.contractId,
		ContractMethod:     methodName,
		ContractInputBytes: types.NewSomeOption(&methodArgs),
	}

	retryIndex := 0

	for retryIndex < constants.MAX_RETRIES {
		response, err := w.ExecuteInner(args, nonceKey, publicKeyHex, fromAddr, toAddr, weilpodCounter)
		if err != nil {
			return nil, err
		}

		decoderTxnResult := json.NewDecoder(bytes.NewReader([]byte(*response)))
		decoderTxnResult.DisallowUnknownFields()

		var txnResult transaction.TransactionResult

		err = decoderTxnResult.Decode(&txnResult)
		if err == nil {
			return &txnResult, nil
		}

		decoderNonceFailure := json.NewDecoder(bytes.NewReader([]byte(*response)))
		decoderNonceFailure.DisallowUnknownFields()

		var nonceFailureResponse NonceFailureResponse

		if decoderNonceFailure.Decode(&nonceFailureResponse) == nil {
			w.client.nonceTracker.SetNonce(nonceKey, uint64(nonceFailureResponse.ExpectedNonce))
			retryIndex += 1

			continue
		}

		return nil, fmt.Errorf("sentinel server returned invalid response : %s", response)
	}

	return nil, fmt.Errorf("failed to submit transaction to the sentinel node ... max retries reached")
}

func (w WeilContractClient) ExecuteInner(args ExecuteArgs, nonceKey string, publicKey string, fromAddr string, toAddr string, weilpodCounter int) (*string, error) {
	nonce := w.client.nonceTracker.GetNonce(nonceKey)
	txnHeader := transaction.NewTransactionHeader(nonce, publicKey, fromAddr, toAddr, weilpodCounter)

	signature, err := w.SignExecuteArgs(txnHeader, &args)

	if err != nil {
		return nil, err
	}

	txnHeader.SetSignature(*signature)
	baseTxn := transaction.NewBaseTransaction(*txnHeader)

	response, err := w.SubmitSignedArgs(*signature, baseTxn, args)

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

func (w WeilContractClient) SubmitSignedArgs(signature string, txn *transaction.BaseTransaction, args ExecuteArgs) (*string, error) {

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
			},
		},
	}

	response, err := api.SubmitTransaction(w.httpClient, payload)

	if err != nil {
		return nil, err
	}

	return response, nil
}
