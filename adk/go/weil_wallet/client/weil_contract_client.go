package client

import (
	"encoding/hex"
	"net/http"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/contract"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

// WeilContractClient is a per-contract view over a WeilClient.
// It pins a specific contractId so callers only need to supply method name
// and arguments when invoking applet methods.
type WeilContractClient struct {
	httpClient *http.Client
	contractId string
	client     *WeilClient
}

// ExecuteArgs holds the canonicalized fields that are signed and submitted
// as part of a SmartContractExecutor transaction.
type ExecuteArgs struct {
	ContractAddress    string
	ContractMethod     string
	ContractInputBytes *types.Option[string]
	ShouldHideArgs     bool
}

// NonceFailureResponse is returned by the platform when a transaction is
// rejected due to a nonce mismatch, allowing the caller to resync.
type NonceFailureResponse struct {
	ExpectedNonce uint32 `json:"expected_nonce"`
	ReceivedNonce uint32 `json:"received_nonce"`
	Message       string `json:"message"`
	Status        string `json:"status"`
}

// orgContext maps the wallet-file OrgInfo onto the wire shape the node expects.
//
// Purpose is intentionally dropped — it is advisory metadata resolved from the
// Identity contract at runtime, not part of the signed claim. An empty subgroup
// is normalized to nil so org-level membership always produces one digest,
// never two.
func orgContext(info *wallet.OrgInfo) *transaction.OrgContext {
	if info == nil {
		return nil
	}

	subgroup := info.Subgroup
	if subgroup != nil && *subgroup == "" {
		subgroup = nil
	}

	return &transaction.OrgContext{
		Org:      info.Name,
		Subgroup: subgroup,
	}
}

// Execute calls the named method on the bound contract and returns the
// transaction result. It builds and signs the transaction header, then
// submits it via the platform API.
//
//   - methodName: the exported applet method to invoke.
//   - methodArgs: JSON-encoded argument payload.
//   - shouldHideArgs: when true the arguments are encrypted before submission.
//   - isNonBlocking: when true the platform responds immediately without
//     waiting for the transaction to be finalized.
func (w *WeilContractClient) Execute(methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	// Snapshot wallet state under the mutex so we don't hold it during network I/O.
	w.client.walletMu.Lock()
	publicKey := w.client.wallet.GetPublicKey()
	fromAddr := w.client.wallet.GetAddress()
	w.client.walletMu.Unlock()

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
	// Stamp the wallet's active org before signing — org is part of the
	// digest, so it must be set ahead of SignExecuteArgs.
	txnHeader := transaction.NewTransactionHeader(nonce, publicKeyHex, fromAddr, toAddr, weilpodCounter, orgContext(w.client.wallet.Org()))

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

// SignExecuteArgs canonicalizes the transaction payload into a sorted BTreeMap,
// serializes it to JSON, and signs the bytes with the client wallet's secp256k1 key.
// Returns the hex-encoded compact (64-byte) ECDSA signature.
func (w WeilContractClient) SignExecuteArgs(txnHeader *transaction.TransactionHeader, args *ExecuteArgs) (*string, error) {
	jsonPayload := map[string]interface{}{
		"nonce":     txnHeader.Nonce,
		"from_addr": txnHeader.FromAddr,
		"to_addr":   txnHeader.ToAddr,
		"salt":      txnHeader.Salt,
		"user_txn": map[string]any{
			"type":                 "SmartContractExecutor",
			"contract_address":     args.ContractAddress,
			"contract_method":      args.ContractMethod,
			"contract_input_bytes": args.ContractInputBytes,
			"should_hide_args":     args.ShouldHideArgs,
		},
	}

	// `org` is included **only when present**, mirroring the node's
	// compute_verify_digest_for_execute. This conditionality preserves
	// compatibility: a wallet with no active org produces exactly the bytes it
	// did before org existed, so already-signed transactions still verify.
	// Writing a nil org unconditionally would change the digest for every
	// org-less transaction at once.
	//
	// Key placement does not matter here — this stays a map[string]interface{},
	// and encoding/json sorts map keys, which is what matches the node's
	// BTreeMap. Promoting this literal to a struct would silently break that.
	if txnHeader.Org != nil {
		jsonPayload["org"] = txnHeader.Org
	}

	jsonPayloadBtreemap := transaction.ValueToBtreeMap(jsonPayload)
	jsonPayloadJson, err := transaction.BtreeMapToJson(jsonPayloadBtreemap)

	if err != nil {
		return nil, err
	}

	w.client.walletMu.Lock()
	signature, err := w.client.wallet.Sign(jsonPayloadJson)
	w.client.walletMu.Unlock()

	if err != nil {
		return nil, err
	}

	return signature, nil
}

// SubmitSignedArgs constructs a SubmitTxnRequest from the signed transaction
// and args, then submits it to the platform API. This is the common submission
// path used by Execute after signing.
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
				txn.Header.Salt,
				txn.Header.Org,
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
