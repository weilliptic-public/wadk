package api

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/internal/constants"
)

func SubmitTransaction(httpClient *http.Client, payload SubmitTxnRequest) (*string, error) {
	jsonData, err := json.Marshal(payload)

	if err != nil {
		return nil, err
	}

	apiURL := fmt.Sprintf("https://%s/submit_txn", constants.SENTINEL_HOST)
	req, err := http.NewRequest("POST", apiURL, bytes.NewBuffer(jsonData))

	if err != nil {
		return nil, err
	}

	req.Header.Set("Content-Type", "application/json")

	resp, err := httpClient.Do(req)

	if err != nil {
		return nil, err
	}

	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)

	if err != nil {
		return nil, err
	}

	var strResponse types.Result[string, string]

	if json.Unmarshal(respBody, &strResponse) != nil {
		return nil, err
	}

	if strResponse.IsErrResult() {
		return nil, fmt.Errorf(*strResponse.TryErrResult())
	}

	return strResponse.TryOkResult(), nil
}
