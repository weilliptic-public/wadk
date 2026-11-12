package weiladk

import (
	"context"
	"sync"
)

// backgroundAuditSlots bounds the audit entries in flight from
// submitAudit, as the client bounds its own submissions.
const backgroundAuditSlots = 64

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

// submitAudit runs an audit entry made outside a plugin run (the A2A
// middleware, MCP tool results) in the background, so the request it
// records does not wait on the chain. The entry waits for the auditor to run
// it: a submission the platform accepted can still be lost before it runs,
// and only waiting tells the two apart; a lost one is resent with the same
// transaction ID (client.DurableRetry). report, if non-nil, receives an
// error that persists. Each entry carries its own timestamp, so entries in
// flight together may land in any order.
func submitAudit(id *Identity, call Call, report func(error)) {
	call.FireAndForget = false
	backgroundMu.Lock()
	backgroundPending++
	backgroundMu.Unlock()
	go func() {
		defer backgroundDone()
		backgroundAuditSlotC <- struct{}{}
		defer func() { <-backgroundAuditSlotC }()
		if _, err := id.Invoke(context.Background(), call); err != nil && report != nil {
			report(err)
		}
	}()
}

// FlushAudits waits until every audit entry the A2A middleware and MCP
// audit have queued has run or failed for good, or ctx ends. Call it before
// a server exits so its last entries are not lost.
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
