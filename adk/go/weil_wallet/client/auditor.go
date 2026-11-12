package client

import (
	"context"
	"crypto/sha256"
	"encoding/hex"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

// AuditorApplet is the service name of the systemic auditor applet.
const AuditorApplet = "auditor::weil"

// Audit submits a log entry to the auditor applet, with its arguments
// public, as the Python SDK's client.audit(log) does, and waits for the
// auditor to run it: a submission the platform accepted can still be lost
// before it runs, and only waiting shows it. A lost entry is resent with
// the same transaction ID (DurableRetry); an entry the auditor rejects is
// returned as a *transaction.AppletError.
func (w *WeilClient) Audit(log string) error {
	res, err := w.AuditContext(context.Background(), log, ExecuteOptions{Retry: DurableRetry})
	if err != nil {
		return err
	}
	return res.Decode(nil)
}

// AuditArgs are the auditor applet's `audit` arguments for log, tagged with
// org (which may be nil): {"log", "org", "subgroup"}, with null for an
// absent org or subgroup, as the Python SDK sends them.
func AuditArgs(log string, org *wallet.OrgInfo) any {
	args := struct {
		Log      string  `json:"log"`
		Org      *string `json:"org"`
		Subgroup *string `json:"subgroup"`
	}{Log: log}
	if org != nil {
		name := org.Name
		args.Org = &name
		args.Subgroup = org.Subgroup
	}
	return args
}

// AuditContext submits a log entry to the auditor applet's `audit` method,
// tagged with the active account's org and subgroup.
func (w *WeilClient) AuditContext(ctx context.Context, log string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	argsBytes, err := PythonDumpsValue(AuditArgs(log, w.Org()), false)
	if err != nil {
		return nil, err
	}
	return w.ExecuteByName(ctx, AuditorApplet, "audit", string(argsBytes), opts)
}

// ReceiptRef locates a receipt persisted by PersistReceipt.
type ReceiptRef struct {
	// Location encodes the storage cloud (high 16 bits) and bucket (low 16 bits).
	Location  uint32 `json:"location"`
	ReceiptID string `json:"receipt_id"`
}

// PersistReceipt stores content as a receipt through the auditor applet's
// validate_and_persist_receipt, which checks the caller's org membership and
// keys the receipt by the SHA-256 of its content.
func (w *WeilClient) PersistReceipt(ctx context.Context, content string) (*ReceiptRef, error) {
	sum := sha256.Sum256([]byte(content))
	args := struct {
		Org      *string `json:"org"`
		Subgroup *string `json:"subgroup"`
		Hash     string  `json:"hash"`
		Content  string  `json:"content"`
	}{Hash: hex.EncodeToString(sum[:]), Content: content}
	if org := w.Org(); org != nil {
		name := org.Name
		args.Org = &name
		args.Subgroup = org.Subgroup
	}

	argsBytes, err := PythonDumpsValue(args, false)
	if err != nil {
		return nil, err
	}
	resp, err := w.ExecuteByName(ctx, AuditorApplet, "validate_and_persist_receipt", string(argsBytes), ExecuteOptions{})
	if err != nil {
		return nil, err
	}

	var ref ReceiptRef
	if err := resp.Decode(&ref); err != nil {
		return nil, err
	}
	return &ref, nil
}

// GetReceiptForCommit reads back the receipt content currently persisted for
// commitHash, or nil if nothing has been persisted for it yet.
//
// Used by the same-turn-commit-gap amend path to fetch the payload an agent-run
// mid-turn commit already shipped — with empty prompts/usage, since that commit
// landed before Stop had computed them — so it can be merged and re-persisted
// under the same commit hash.
func (w *WeilClient) GetReceiptForCommit(commitHash string) (*string, error) {
	return w.GetReceiptForCommitContext(context.Background(), commitHash)
}

// GetReceiptForCommitContext is GetReceiptForCommit with a context.
func (w *WeilClient) GetReceiptForCommitContext(ctx context.Context, commitHash string) (*string, error) {
	args := struct {
		CommitHash string `json:"commit_hash"`
	}{CommitHash: commitHash}

	argsBytes, err := PythonDumpsValue(args, false)
	if err != nil {
		return nil, err
	}

	resp, err := w.ExecuteByName(ctx, AuditorApplet, "get_receipt_for_commit", string(argsBytes), ExecuteOptions{})
	if err != nil {
		return nil, err
	}

	var receipt *string
	if err := resp.Decode(&receipt); err != nil {
		return nil, err
	}
	return receipt, nil
}
