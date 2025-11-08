package utils

import (
	"encoding/json"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
)

func TryIntoResult[T any](
	result *types.Result[string, errors.WeilError],
) *types.Result[T, string] {
	if result.IsErrResult() {
		err := *result.TryErrResult()
		errMsg := err.Error()

		return types.NewErrResult[T](&errMsg)
	}

	var ok T
	data := *result.TryOkResult()

	err := json.Unmarshal([]byte(data), &ok)

	if err != nil {
		errMsg := err.Error()

		return types.NewErrResult[T](&errMsg)
	}

	return types.NewOkResult[T, string](&ok)
}
