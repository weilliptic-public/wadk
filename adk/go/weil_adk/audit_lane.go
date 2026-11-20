package weiladk

import (
	"context"
	"errors"
	"sync"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// backgroundAuditSlots bounds the audit entries in flight from
// submitAudit, as the client bounds its own submissions.
const backgroundAuditSlots = 64

// AuditFlushTimeout is how long the end of an agent's run waits for its
// audit entries: as long as a signed transaction stays valid, after which an
// entry has failed for good.
const AuditFlushTimeout = client.SignedTxnTTL

var (
	backgroundAuditSlotC = make(chan struct{}, backgroundAuditSlots)

	// Entries submitted and not yet finished, and the flushes waiting for
	// none to be. A WaitGroup cannot serve: entries keep arriving while a
	// flush waits.
	backgroundMu      sync.Mutex
	backgroundPending int
	backgroundIdle    []chan struct{}
)

func backgroundDone() {
	backgroundMu.Lock()
	defer backgroundMu.Unlock()
	if backgroundPending--; backgroundPending == 0 {
		for _, c := range backgroundIdle {
			close(c)
		}
		backgroundIdle = nil
	}
}

// submitAudit runs an audit entry in the background, so the agent, tool or
// request it records does not wait on the chain. The entry waits for the
// auditor to run it: a submission the platform accepted can still be lost
// before it runs, and only waiting tells the two apart; a lost one is resent
// with the same transaction ID (client.DurableRetry). An entry the auditor
// rejects, or that fails for good, goes to report, if non-nil. Each entry
// carries its own timestamp, so entries in flight together may land in any
// order. The end of each agent's run waits for all of them (FlushAudits).
func submitAudit(id *Identity, call Call, report func(error)) {
	call.FireAndForget = false
	backgroundMu.Lock()
	backgroundPending++
	backgroundMu.Unlock()
	go func() {
		defer backgroundDone()
		backgroundAuditSlotC <- struct{}{}
		defer func() { <-backgroundAuditSlotC }()
		res, err := id.Invoke(context.Background(), call)
		if err == nil && res != nil && res.Txn != nil {
			err = transaction.RejectedBy(res.Txn.TxnResult)
		}
		if err != nil && report != nil {
			report(err)
		}
	}()
}

// FlushAudits waits until every audit entry submitted so far — agent and
// tool records, the A2A middleware's, MCP results' — has run or failed for
// good, or ctx ends. The end of each agent's run under WithSecureA2A waits
// this way, up to AuditFlushTimeout; call it before a server exits so its
// last entries are not lost.
func FlushAudits(ctx context.Context) error {
	backgroundMu.Lock()
	if backgroundPending == 0 {
		backgroundMu.Unlock()
		return nil
	}
	done := make(chan struct{})
	backgroundIdle = append(backgroundIdle, done)
	backgroundMu.Unlock()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// flushAuditsFor waits up to AuditFlushTimeout for the pending audit
// entries, and reports through warn if some are still pending then; they
// carry on in the background.
func flushAuditsFor(warn func(error)) {
	ctx, cancel := context.WithTimeout(context.Background(), AuditFlushTimeout)
	defer cancel()
	if err := FlushAudits(ctx); err != nil {
		warn(errAuditsPending)
	}
}

var errAuditsPending = errors.New("weiladk: audit entries still pending at agent end")
