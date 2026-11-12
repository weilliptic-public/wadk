package api

import (
	"bytes"
	"compress/gzip"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/transaction"
)

// maxErrorBody caps how much of a failed response body is quoted in errors.
const maxErrorBody = 500 // as the Python SDK truncates

// HTTPError is returned when Sentinel answers with a non-2xx status.
type HTTPError struct {
	Endpoint   string
	StatusCode int
	Body       string
}

func (e *HTTPError) Error() string {
	if e.Endpoint == "execute_smartcontract" {
		// The Python SDK's wording.
		return fmt.Sprintf("failed to submit the transaction: HTTP %d %s", e.StatusCode, e.Body)
	}
	if e.Body == "" {
		return fmt.Sprintf("%s failed: HTTP %d", e.Endpoint, e.StatusCode)
	}
	return fmt.Sprintf("%s failed: HTTP %d: %s", e.Endpoint, e.StatusCode, e.Body)
}

// ParseError reports a 2xx reply to a submission that could not be read or
// parsed: the platform accepted the call, so it is not one to resend.
type ParseError struct {
	Err  error
	Body string
}

func (e *ParseError) Error() string {
	return fmt.Sprintf("failed to parse transaction result: %v (body: %s)", e.Err, e.Body)
}

func (e *ParseError) Unwrap() error { return e.Err }

// newSubmitRequest builds the multipart POST to
// /contracts/execute_smartcontract carrying the gzipped transaction.
func newSubmitRequest(ctx context.Context, sentinelHost string, payload SubmitTxnRequest, isNonBlocking bool) (*http.Request, error) {
	payloadJson, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}

	var buf bytes.Buffer
	w := gzip.NewWriter(&buf)

	if _, err := w.Write(payloadJson); err != nil {
		return nil, fmt.Errorf("payload compression failed: %w", err)
	}

	if err := w.Close(); err != nil {
		return nil, fmt.Errorf("payload compression failed: %w", err)
	}

	body := &bytes.Buffer{}
	writer := multipart.NewWriter(body)

	part, err := writer.CreatePart(textproto.MIMEHeader{
		"Content-Disposition": []string{`form-data; name="transaction"; filename="transaction_data"`},
		"Content-Type":        []string{"application/octet-stream"},
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create multipart part: %w", err)
	}

	if _, err := part.Write(buf.Bytes()); err != nil {
		return nil, fmt.Errorf("failed to write multipart part: %w", err)
	}

	if err := writer.Close(); err != nil {
		return nil, fmt.Errorf("failed to finalize multipart body: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost,
		fmt.Sprintf("%s/contracts/execute_smartcontract", sentinelHost),
		body,
	)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", writer.FormDataContentType())

	if isNonBlocking {
		req.Header.Set("x-non-blocking", "true")
	}
	// Route every call from one wallet to the same pod member: the member
	// that ran a write and reported its result has applied it, so the
	// wallet's next read sees it. Other members may still be catching up.
	if t := payload.Transaction; t != nil && t.TxnHeader != nil && t.TxnHeader.FromAddr != "" {
		req.Header.Set("x-sticky-key", t.TxnHeader.FromAddr)
	}

	return req, nil
}

// SubmitTransaction submits a signed transaction to Sentinel's
// /contracts/execute_smartcontract endpoint.
//
// With isNonBlocking the platform acknowledges the submission without waiting
// for execution, and the returned result carries only the TxnTicket to look
// the transaction up with (see TxnStatus), on platforms that return one.
func SubmitTransaction(ctx context.Context, httpClient *http.Client, sentinelHost string, payload SubmitTxnRequest, isNonBlocking bool) (*transaction.TransactionResult, error) {
	req, err := newSubmitRequest(ctx, sentinelHost, payload, isNonBlocking)
	if err != nil {
		return nil, err
	}

	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		if resp.StatusCode >= 200 && resp.StatusCode < 300 {
			// The platform accepted the call; only its reply was lost.
			return nil, &ParseError{Err: fmt.Errorf("failed to read response: %w", err)}
		}
		return nil, &HTTPError{Endpoint: "execute_smartcontract", StatusCode: resp.StatusCode}
	}

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		// The platform reports an applet method that returned an error as
		// HTTP 417 {"status": "failure", "message": ...}; surface it as an
		// applet error so callers can tell it apart from transport failures.
		if resp.StatusCode == http.StatusExpectationFailed {
			var failure struct {
				Message string `json:"message"`
			}
			if json.Unmarshal(respBody, &failure) == nil && failure.Message != "" {
				return nil, &transaction.AppletError{Message: failure.Message}
			}
		}
		return nil, &HTTPError{Endpoint: "execute_smartcontract", StatusCode: resp.StatusCode, Body: truncate(respBody)}
	}

	var txnResult transaction.TransactionResult
	if err := json.Unmarshal(respBody, &txnResult); err != nil {
		return nil, &ParseError{Err: err, Body: truncate(respBody)}
	}
	// An unknown status reads as InProgress, as in the Python SDK.
	switch txnResult.Status {
	case transaction.StatusInProgress, transaction.StatusConfirmed, transaction.StatusFinalized, transaction.StatusFailed:
	default:
		txnResult.Status = transaction.StatusInProgress
	}
	return &txnResult, nil
}

// PostJSON POSTs body as JSON to sentinelHost/endpoint and decodes the JSON
// response into out (which may be nil to discard it).
func PostJSON(ctx context.Context, httpClient *http.Client, sentinelHost string, endpoint string, body any, out any) error {
	respBody, err := PostJSONRaw(ctx, httpClient, sentinelHost, endpoint, body)
	if err != nil {
		return err
	}

	if out == nil {
		return nil
	}
	// The body is deliberately not quoted here: some endpoints (e.g.
	// get_agent_wallet) return key material on success.
	if err := json.Unmarshal(respBody, out); err != nil {
		return fmt.Errorf("%s: decode response: %w", endpoint, err)
	}
	return nil
}

// PostJSONRaw POSTs body as JSON to sentinelHost/endpoint and returns the raw
// response body of a 2xx answer.
func PostJSONRaw(ctx context.Context, httpClient *http.Client, sentinelHost string, endpoint string, body any) ([]byte, error) {
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("%s request: %w", endpoint, err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost,
		fmt.Sprintf("%s/%s", sentinelHost, endpoint), bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("%s request: %w", endpoint, err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", endpoint, err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("%s: read response: %w", endpoint, err)
	}

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, &HTTPError{Endpoint: endpoint, StatusCode: resp.StatusCode, Body: truncate(respBody)}
	}
	return respBody, nil
}

func truncate(b []byte) string {
	if len(b) > maxErrorBody {
		return string(b[:maxErrorBody])
	}
	return string(b)
}

// SubmitTransactionStream submits a signed transaction to a method that
// streams its result and returns the response body as it arrives — the
// Python SDK's submit_transaction_with_streaming. The caller closes it.
func SubmitTransactionStream(ctx context.Context, httpClient *http.Client, sentinelHost string, payload SubmitTxnRequest) (io.ReadCloser, error) {
	req, err := newSubmitRequest(ctx, sentinelHost, payload, false)
	if err != nil {
		return nil, err
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		defer resp.Body.Close()
		respBody, _ := io.ReadAll(resp.Body)
		return nil, &HTTPError{Endpoint: "execute_smartcontract", StatusCode: resp.StatusCode, Body: truncate(respBody)}
	}
	return resp.Body, nil
}
