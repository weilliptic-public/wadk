package client

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/api"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// SignedTxnTTL is how long the platform accepts a signed transaction: a
// block drops it DEFAULT_TRANSACTION_TTL (5 minutes) after its nonce. A
// retry resends the same signed transaction only within this window, less
// a margin for the block to form.
const SignedTxnTTL = 5 * time.Minute

const ttlMargin = 30 * time.Second

// txnValidity is how long after signing a transaction is still sent; a
// variable so tests can shorten it.
var txnValidity = SignedTxnTTL - ttlMargin

// RetryPolicy resends a call that failed in transit. The call is signed once
// and every attempt submits those same bytes, so the platform sees one
// transaction ID however many attempts it takes.
//
// Only failures where the call may not have reached the chain are retried:
// network errors, attempt timeouts, HTTP 408, 425, 429 and 5xx. An applet
// error or any other HTTP status is returned at once. The platform does not
// deduplicate transactions, so a call that was accepted but whose reply was
// lost runs again — with the same transaction ID, which tells the copies
// apart from distinct calls. Retry only methods that tolerate that, such as
// appending an audit entry.
type RetryPolicy struct {
	// MaxAttempts counts the first try; 0 or 1 disables retries.
	MaxAttempts int
	// InitialBackoff is the wait before the second attempt, doubling up to
	// MaxBackoff. Defaults 250ms and 10s.
	InitialBackoff time.Duration
	MaxBackoff     time.Duration
	// AttemptTimeout bounds each attempt. 0 lets an attempt run until the
	// signed transaction expires (ctx and the HTTP client's own timeout
	// aside): a congested Sentinel holds requests in its queue, and
	// abandoning one there only sends it to the back — while the abandoned
	// copy may still run.
	AttemptTimeout time.Duration
}

// DurableRetry is the policy for calls that must not be lost under load,
// such as audit entries. Each attempt waits as long as Sentinel takes to
// accept the call, up to the signed transaction's expiry; a failed attempt
// (connection refused or reset, HTTP 429 or 5xx) is resent with backoff
// until one is accepted, ctx ends or the transaction expires.
var DurableRetry = RetryPolicy{
	MaxAttempts:    1 << 20,
	InitialBackoff: 250 * time.Millisecond,
	MaxBackoff:     10 * time.Second,
}

func (p RetryPolicy) enabled() bool { return p.MaxAttempts > 1 }

// maxResigns bounds how often a durable call is signed afresh because its
// transaction expired before any attempt could be sent (all of the client's
// submission slots stayed busy): an unsent transaction can be replaced
// safely, as nothing of it reached the platform.
const maxResigns = 3

// errNotSent reports that a transaction expired before any attempt to send
// it began.
var errNotSent = errors.New("weil_wallet: the transaction expired before it could be sent")

// executeWithRetry signs and submits a call under opts.Retry.
func (w *WeilContractClient) executeWithRetry(ctx context.Context, methodName, methodArgs string, opts ExecuteOptions) (*transaction.TransactionResult, error) {
	for resigns := 0; ; resigns++ {
		signature, txn, args, signedAt, err := w.signWithRetry(ctx, methodName, methodArgs, opts)
		if err != nil {
			return nil, err
		}
		res, err := w.submitWithRetry(ctx, signature, txn, args, opts, signedAt)
		if errors.Is(err, errNotSent) && resigns < maxResigns && ctx.Err() == nil {
			continue
		}
		return res, err
	}
}

// signWithRetry signs a call, retrying under opts.Retry when a remote
// signer could not be reached: nothing has been submitted yet, so signing
// again is always safe. Retries stop after SignedTxnTTL.
func (w *WeilContractClient) signWithRetry(ctx context.Context, methodName, methodArgs string, opts ExecuteOptions) (string, *transaction.BaseTransaction, *ExecuteArgs, time.Time, error) {
	if !opts.Retry.enabled() {
		signedAt := time.Now()
		sig, txn, args, err := w.sign(ctx, methodName, methodArgs, opts)
		return sig, txn, args, signedAt, err
	}
	signCtx, cancel := context.WithTimeout(ctx, txnValidity)
	defer cancel()
	backoff, maxBackoff := backoffs(opts.Retry)
	for attempt := 1; ; attempt++ {
		signedAt := time.Now()
		sig, txn, args, err := w.sign(signCtx, methodName, methodArgs, opts)
		if err == nil || attempt >= opts.Retry.MaxAttempts || signCtx.Err() != nil || !TransportFailure(err) {
			return sig, txn, args, signedAt, err
		}
		select {
		case <-time.After(backoff):
		case <-signCtx.Done():
			return sig, txn, args, signedAt, err
		}
		backoff = min(backoff*2, maxBackoff)
	}
}

func backoffs(p RetryPolicy) (initial, max time.Duration) {
	initial, max = p.InitialBackoff, p.MaxBackoff
	if initial <= 0 {
		initial = 250 * time.Millisecond
	}
	if max <= 0 {
		max = 10 * time.Second
	}
	return initial, max
}

// TransportFailure reports an error from not reaching Sentinel, or not
// hearing back: a network error or timeout, or HTTP 408, 425, 429 or 5xx.
func TransportFailure(err error) bool {
	var httpErr *api.HTTPError
	if errors.As(err, &httpErr) {
		return retryableStatus(httpErr.StatusCode)
	}
	var urlErr *url.Error
	var netErr net.Error
	return errors.As(err, &urlErr) || errors.As(err, &netErr) || errors.Is(err, io.ErrUnexpectedEOF)
}

func retryableStatus(s int) bool {
	return s == http.StatusRequestTimeout || s == http.StatusTooEarly || s == http.StatusTooManyRequests || s >= 500
}

// submitWithRetry submits a signed call under policy, every attempt within
// the transaction's validity. signedAt is when the call was signed.
func (w *WeilContractClient) submitWithRetry(ctx context.Context, signature string, txn *transaction.BaseTransaction, args *ExecuteArgs, opts ExecuteOptions, signedAt time.Time) (*transaction.TransactionResult, error) {
	p := opts.Retry
	// Built once: every attempt carries the same bytes, creation_time
	// included, which the platform's TTL counts from.
	payload := submitPayload(signature, txn, args)
	if !p.enabled() {
		res, _, err := w.submitAttempt(ctx, w.httpClient, payload, opts.NonBlocking, p.AttemptTimeout)
		return res, err
	}

	backoff, maxBackoff := backoffs(p)
	deadline := signedAt.Add(txnValidity)
	httpClient := w.httpClient
	if p.AttemptTimeout <= 0 {
		// Wait out a congested Sentinel until the transaction expires,
		// rather than the HTTP client's per-request timeout.
		c := *w.httpClient
		c.Timeout = 0
		httpClient = &c
	}

	everSent := false
	for attempt := 1; ; attempt++ {
		// The whole attempt, waiting for a submission slot included, ends
		// when the transaction stops being valid.
		attemptCtx, cancel := context.WithDeadline(ctx, deadline)
		res, sent, err := w.submitAttempt(attemptCtx, httpClient, payload, opts.NonBlocking, p.AttemptTimeout)
		cancel()
		everSent = everSent || sent
		if err == nil || ctx.Err() != nil {
			return res, err
		}
		if !sent && !everSent && !time.Now().Before(deadline) {
			return nil, fmt.Errorf("%w: %v", errNotSent, err)
		}
		if attempt >= p.MaxAttempts || !Retryable(err) {
			return res, err
		}
		if time.Now().Add(backoff).After(deadline) {
			if !everSent {
				return nil, fmt.Errorf("%w: %v", errNotSent, err)
			}
			return res, err
		}
		select {
		case <-time.After(backoff):
		case <-ctx.Done():
			return res, err
		}
		backoff = min(backoff*2, maxBackoff)
	}
}

// submitAttempt submits once, reporting whether the request was sent.
// Waiting for a submission slot is bounded by ctx; timeout, if set, bounds
// the request itself.
func (w *WeilContractClient) submitAttempt(ctx context.Context, httpClient *http.Client, payload api.SubmitTxnRequest, nonBlocking bool, timeout time.Duration) (*transaction.TransactionResult, bool, error) {
	if err := w.client.acquire(ctx); err != nil {
		return nil, false, err
	}
	defer w.client.release()
	if err := ctx.Err(); err != nil {
		return nil, false, err
	}
	if timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}
	res, err := api.SubmitTransaction(ctx, httpClient, w.client.sentinelHost, payload, nonBlocking)
	return res, true, err
}

// Retryable reports whether err, from submitting a call, leaves open that
// the call never reached the chain, so resending it may be needed: a
// transport failure (see TransportFailure) or the platform timing out
// (see PlatformTimeout). An applet error, a reply that could not be read or
// parsed (the call was accepted) and any other failure are final.
func Retryable(err error) bool {
	if err == nil {
		return false
	}
	var appletErr *transaction.AppletError
	if errors.As(err, &appletErr) {
		return PlatformTimeout(appletErr)
	}
	var parseErr *api.ParseError
	if errors.As(err, &parseErr) {
		return false
	}
	return TransportFailure(err)
}

// PlatformTimeout reports whether an applet error is in fact the platform
// giving up on waiting for the transaction to finish (its TXN_TIMEOUT,
// tokio's "deadline has elapsed"): Sentinel answers that with the same HTTP
// 417 as an applet rejecting the call. The call may still run, as after any
// timeout, so it is retried like one.
func PlatformTimeout(err *transaction.AppletError) bool {
	return err != nil && strings.Contains(err.Message, "deadline has elapsed")
}
