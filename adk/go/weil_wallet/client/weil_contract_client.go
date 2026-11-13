package client

import (
	"context"
	"encoding/hex"
	"io"
	"net/http"
	"sync"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/contract"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
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

// Execute calls methodName on the bound applet.
//
// Deprecated: use ExecuteContext, which honours cancellation.
func (w *WeilContractClient) Execute(methodName string, methodArgs string, shouldHideArgs bool, isNonBlocking bool) (*transaction.TransactionResult, error) {
	return w.ExecuteContext(context.Background(), methodName, methodArgs,
		ExecuteOptions{HideArgs: shouldHideArgs, NonBlocking: isNonBlocking})
}

// ExecuteContext signs and submits a call of methodName with JSON-encoded
// methodArgs on the bound applet, as the client's active account.
func (w *WeilContractClient) ExecuteContext(ctx context.Context, methodName string, methodArgs string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	return w.executeWithRetry(ctx, methodName, methodArgs, opts)
}

// ExecuteStream signs and submits a call of a method that streams its result
// (an applet method returning a stream, e.g. an LLM chat) and returns the
// response body as it arrives — Python's execute_with_streaming. Arguments
// are public, as in Python. The caller reads and closes the stream.
func (w *WeilContractClient) ExecuteStream(ctx context.Context, methodName string, methodArgs string) (io.ReadCloser, error) {
	signature, baseTxn, args, err := w.sign(ctx, methodName, methodArgs, ExecuteOptions{})
	if err != nil {
		return nil, err
	}
	if err := w.client.acquire(ctx); err != nil {
		return nil, err
	}
	// A stream may last longer than any whole-request timeout: bound each
	// read instead (DefaultTimeout without data), as httpx's read timeout
	// does, and keep the concurrency slot until the stream is closed, as
	// Python holds its semaphore for the whole iteration.
	ctx, cancel := context.WithCancel(ctx)
	streamClient := *w.httpClient
	streamClient.Timeout = 0
	body, err := api.SubmitTransactionStream(ctx, &streamClient, w.client.sentinelHost, submitPayload(signature, baseTxn, args))
	if err != nil {
		cancel()
		w.client.release()
		return nil, err
	}
	return newIdleStream(body, DefaultTimeout, cancel, w.client.release), nil
}

// idleStream fails a stream that sends nothing for idle, and runs its
// cleanups once on Close.
type idleStream struct {
	body     io.ReadCloser
	idle     time.Duration
	timer    *time.Timer
	cancel   context.CancelFunc
	release  func()
	closeOne sync.Once
}

func newIdleStream(body io.ReadCloser, idle time.Duration, cancel context.CancelFunc, release func()) *idleStream {
	s := &idleStream{body: body, idle: idle, cancel: cancel, release: release}
	s.timer = time.AfterFunc(idle, cancel)
	return s
}

func (s *idleStream) Read(p []byte) (int, error) {
	n, err := s.body.Read(p)
	if n > 0 {
		s.timer.Reset(s.idle)
	}
	return n, err
}

func (s *idleStream) Close() error {
	err := s.body.Close()
	s.closeOne.Do(func() {
		s.timer.Stop()
		s.cancel()
		s.release()
	})
	return err
}

// sign builds and signs the transaction for a call of methodName.
func (w *WeilContractClient) sign(ctx context.Context, methodName string, methodArgs string, opts ExecuteOptions) (string, *transaction.BaseTransaction, *ExecuteArgs, error) {
	// Snapshot the account once so the header and the signature always agree,
	// even if SetAccount runs concurrently. No lock is held during network I/O.
	account := w.client.CurrentAccount()
	publicKey := account.GetPublicKey()
	fromAddr := account.GetAddress()

	toAddr := fromAddr
	weilpodCounter, err := contract.PodCounter(w.contractId)
	if err != nil {
		return "", nil, nil, err
	}

	publicKeyHex := hex.EncodeToString(publicKey.SerializeUncompressed())

	args := &ExecuteArgs{
		ContractAddress:    w.contractId,
		ContractMethod:     methodName,
		ContractInputBytes: types.NewSomeOption(&methodArgs),
		ShouldHideArgs:     opts.HideArgs,
	}

	nonce := int(time.Now().UnixMilli())
	// Stamp the account's active org before signing — org is part of the
	// digest, so it must be set ahead of signing.
	txnHeader := transaction.NewTransactionHeader(nonce, publicKeyHex, fromAddr, toAddr, weilpodCounter, orgContext(account.Org()))

	signature, err := w.signExecuteArgs(ctx, account, txnHeader, args)
	if err != nil {
		return "", nil, nil, err
	}

	txnHeader.SetSignature(signature)
	return signature, transaction.NewBaseTransaction(txnHeader), args, nil
}

// executePayload builds the object a transaction's signature covers.
func executePayload(txnHeader *transaction.TransactionHeader, args *ExecuteArgs) map[string]any {
	payload := map[string]any{
		"nonce":     txnHeader.Nonce,
		"from_addr": txnHeader.FromAddr,
		"to_addr":   txnHeader.ToAddr,
		"salt":      txnHeader.Salt,
		"user_txn": map[string]any{
			"type":                 "SmartContractExecutor",
			"contract_address":     args.ContractAddress,
			"contract_method":      args.ContractMethod,
			"contract_input_bytes": inputBytes(args.ContractInputBytes),
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
	// Key placement does not matter here — this stays a map[string]any, and
	// encoding/json sorts map keys, which is what matches the node's BTreeMap.
	// Promoting this literal to a struct would silently break that.
	if txnHeader.Org != nil {
		payload["org"] = txnHeader.Org
	}

	return payload
}

// inputBytes unwraps the method args for the signed payload. types.Option
// marshals its value with json.Marshal, which escapes <, > and & (\u003c ...)
// before the outer encoder sees them; the node hashes them unescaped, so args
// holding any of them would fail verification.
func inputBytes(o *types.Option[string]) any {
	if o == nil || !o.IsSomeResult() {
		return nil
	}
	return *o.TrySomeOption()
}

func (w *WeilContractClient) signExecuteArgs(ctx context.Context, account *wallet.Account, txnHeader *transaction.TransactionHeader, args *ExecuteArgs) (string, error) {
	signer, err := w.client.signerFor(account)
	if err != nil {
		return "", err
	}
	return signer.Sign(ctx, account, executePayload(txnHeader, args))
}

// SignExecuteArgs signs the execute payload for txnHeader and args with the
// client's active account.
func (w WeilContractClient) SignExecuteArgs(txnHeader *transaction.TransactionHeader, args *ExecuteArgs) (*string, error) {
	signature, err := w.signExecuteArgs(context.Background(), w.client.CurrentAccount(), txnHeader, args)
	if err != nil {
		return nil, err
	}
	return &signature, nil
}

// SubmitSignedArgs submits a transaction already signed with signature.
func (w WeilContractClient) SubmitSignedArgs(signature string, txn *transaction.BaseTransaction, args *ExecuteArgs, isNonBlocking bool) (*transaction.TransactionResult, error) {
	return w.submitSignedArgs(context.Background(), signature, txn, args, isNonBlocking)
}

func (w *WeilContractClient) submitSignedArgs(ctx context.Context, signature string, txn *transaction.BaseTransaction, args *ExecuteArgs, isNonBlocking bool) (*transaction.TransactionResult, error) {
	res, _, err := w.submitAttempt(ctx, w.httpClient, submitPayload(signature, txn, args), isNonBlocking, 0)
	return res, err
}

// submitPayload is the request carrying a signed call.
func submitPayload(signature string, txn *transaction.BaseTransaction, args *ExecuteArgs) api.SubmitTxnRequest {
	typeStr := "SmartContractExecutor"
	return api.SubmitTxnRequest{
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
}
