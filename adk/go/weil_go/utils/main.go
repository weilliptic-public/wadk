// Package utils provides utility functions for Weil contracts.
package utils

import (
	"encoding/json"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

// TryIntoResult attempts to convert a Result[string, WeilError] into a Result[T, string]
// by deserializing the string value into type T.
// If the original result is an error, it is converted to a string error.
// If deserialization fails, an error result is returned.
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
