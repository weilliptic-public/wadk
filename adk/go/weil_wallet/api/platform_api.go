package api

import (
	"bytes"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"

	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/internal/constants"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/transaction"
)

func SubmitTransaction(httpClient *http.Client, payload SubmitTxnRequest, isNonBlocking bool) (*transaction.TransactionResult, error) {
	// Compress payload
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

	// Build multipart form
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

	writer.Close()

	// Build request
	req, err := http.NewRequest(http.MethodPost,
		fmt.Sprintf("https://%s/contracts/execute_smartcontract", constants.SENTINEL_HOST),
		body,
	)

	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", writer.FormDataContentType())

	if isNonBlocking {
		req.Header.Set("x-non-blocking", "true")
	}

	resp, err := httpClient.Do(req)

	if err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("failed to submit the transaction")
	}

	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)

	if err != nil {
		return nil, err
	}

	var txnResult transaction.TransactionResult

	if isNonBlocking {
		return &transaction.TransactionResult{}, nil
	}

	if json.Unmarshal(respBody, &txnResult) != nil {
		return nil, err
	}

	return &txnResult, nil
}
