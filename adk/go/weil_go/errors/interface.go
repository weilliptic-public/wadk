// Package errors provides error types and utilities for Weil contracts.
// It defines the WeilError interface and various error implementations
// that can occur during contract execution.
package errors

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal/constants"
)

// WeilError is the interface that all Weil error types must implement.
// It extends the standard error interface with an ErrorName method
// that returns the error type name.
type WeilError interface {
	ErrorName() string
	error
}

// MarshalJSONWasmHostInterfaceError marshals a WeilError to JSON format
// for communication with the WASM host interface.
func MarshalJSONWasmHostInterfaceError(err WeilError) []byte {
	tmp := make(map[string]any)

	switch err := err.(type) {
	case *MethodArgumentDeserializationError:
		tmp[err.ErrorName()] = err.inner
	case *FunctionReturnedWithError:
		tmp[err.ErrorName()] = err.inner
	case *TrapOccuredWhileWasmModuleExecutionError:
		tmp[err.ErrorName()] = err.inner
	case *KeyNotFoundInCollectionError:
		tmp[err.ErrorName()] = err.key
	case *NoValueReturnedFromDeletingCollectionItemError:
		tmp[err.ErrorName()] = err.key
	case *EntriesNotFoundInCollectionForKeysWithPrefixError:
		tmp[err.ErrorName()] = err.key
	case *ContractMethodExecutionError:
		tmp[err.ErrorName()] = err.inner
	case *InvalidCrossContractCallError:
		tmp[err.ErrorName()] = err.inner
	case *CrossContractCallResultDeserializationError:
		tmp[err.ErrorName()] = err.inner
	case *LLMClusterError:
		tmp[err.ErrorName()] = err.msg
	case *StreamingResponseDeserializationError:
		tmp[err.ErrorName()] = err.msg
	case *OutcallError:
		tmp[err.ErrorName()] = err.msg
	case *InvalidDataReceivedError:
		tmp[err.ErrorName()] = err.msg
	case *InvalidWasmModuleError:
		tmp[err.ErrorName()] = err.msg
	default:
		panic(constants.UNREACHABLE)
	}

	serializedErr, _ := json.Marshal(tmp)

	return serializedErr
}

// WasmHostInterfaceErrorFromBytes unmarshals a WeilError from JSON bytes
// received from the WASM host interface.
func WasmHostInterfaceErrorFromBytes(buffer []byte) WeilError {
	var tmp map[string]interface{}
	_ = json.Unmarshal(buffer, &tmp)

	errorNames := []string{
		"MethodArgumentDeserializationError",
		"FunctionReturnedWithError",
		"TrapOccuredWhileWasmModuleExecution",
		"KeyNotFoundInCollection",
		"NoValueReturnedFromDeletingCollectionItem",
		"EntriesNotFoundInCollectionForKeysWithPrefix",
		"ContractMethodExecutionError",
		"InvalidCrossContractCallError",
		"CrossContractCallResultDeserializationError",
		"LLMClusterError",
		"StreamingResponseDeserializationError",
		"OutcallError",
		"InvalidDataReceivedError",
		"InvalidWasmModuleError",
	}

	var criticalErrorName string

	for _, errorName := range errorNames {
		_, ok := tmp[errorName]
		if ok {
			criticalErrorName = errorName
			break
		}
	}

	if criticalErrorName == "" {
		panic(constants.UNREACHABLE)
	}

	serializedEntry, _ := json.Marshal(tmp[criticalErrorName])

	switch criticalErrorName {
	case "MethodArgumentDeserializationError":
		var inner MethodError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &MethodArgumentDeserializationError{
			inner: &inner,
		}
	case "FunctionReturnedWithError":
		var inner MethodError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &FunctionReturnedWithError{
			inner: &inner,
		}
	case "TrapOccuredWhileWasmModuleExecution":
		var inner MethodError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &TrapOccuredWhileWasmModuleExecutionError{
			inner: &inner,
		}
	case "KeyNotFoundInCollection":
		var key string
		_ = json.Unmarshal(serializedEntry, &key)

		return &KeyNotFoundInCollectionError{
			key: key,
		}
	case "NoValueReturnedFromDeletingCollectionItem":
		var key string
		_ = json.Unmarshal(serializedEntry, &key)

		return &NoValueReturnedFromDeletingCollectionItemError{
			key: key,
		}
	case "EntriesNotFoundInCollectionForKeysWithPrefix":
		var key string
		_ = json.Unmarshal(serializedEntry, &key)

		return &EntriesNotFoundInCollectionForKeysWithPrefixError{
			key: key,
		}
	case "ContractMethodExecutionError":
		var inner CrossContractCallError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &ContractMethodExecutionError{
			inner: &inner,
		}
	case "InvalidCrossContractCallError":
		var inner CrossContractCallError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &InvalidCrossContractCallError{
			inner: &inner,
		}
	case "CrossContractCallResultDeserializationError":
		var inner CrossContractCallError
		_ = json.Unmarshal(serializedEntry, &inner)

		return &CrossContractCallResultDeserializationError{
			inner: &inner,
		}
	case "LLMClusterError":
		var msg string
		_ = json.Unmarshal(serializedEntry, &msg)

		return &LLMClusterError{
			msg: msg,
		}
	case "StreamingResponseDeserializationError":
		var msg string
		_ = json.Unmarshal(serializedEntry, &msg)

		return &StreamingResponseDeserializationError{
			msg: msg,
		}
	case "OutcallError":
		var msg string
		_ = json.Unmarshal(serializedEntry, &msg)

		return &OutcallError{
			msg: msg,
		}
	case "InvalidDataReceivedError":
		var msg string
		_ = json.Unmarshal(serializedEntry, &msg)

		return &InvalidDataReceivedError{
			msg: msg,
		}
	case "InvalidWasmModuleError":
		var msg string
		_ = json.Unmarshal(serializedEntry, &msg)

		return &InvalidWasmModuleError{
			msg: msg,
		}
	default:
		panic(constants.UNREACHABLE)
	}
}

// MethodError represents an error that occurred during method execution.
type MethodError struct {
	MethodName string `json:"method_name"`
	ErrMsg     string `json:"err_msg"`
}

// CrossContractCallError represents an error that occurred during a cross-contract call.
type CrossContractCallError struct {
	ContractId string `json:"contract_id"`
	MethodName string `json:"method_name"`
	ErrMsg     string `json:"err_msg"`
}

// MethodArgumentDeserializationError indicates that method arguments could not be deserialized.
type MethodArgumentDeserializationError struct {
	inner *MethodError
}

// NewMethodArgumentDeserializationError creates a new MethodArgumentDeserializationError.
func NewMethodArgumentDeserializationError(methodName string, err error) *MethodArgumentDeserializationError {
	return &MethodArgumentDeserializationError{
		inner: &MethodError{
			MethodName: methodName,
			ErrMsg:     err.Error(),
		},
	}
}

// ErrorName returns the error type name.
func (e *MethodArgumentDeserializationError) ErrorName() string {
	return "MethodArgumentDeserializationError"
}

// Error returns the error message.
func (e *MethodArgumentDeserializationError) Error() string {
	return fmt.Sprintf("arguments for method `%s` cannot be deserialized: `%s`", e.inner.MethodName, e.inner.ErrMsg)
}

// FunctionReturnedWithError indicates that a function returned with an error.
type FunctionReturnedWithError struct {
	inner *MethodError
}

// NewFunctionReturnedWithError creates a new FunctionReturnedWithError.
func NewFunctionReturnedWithError(methodName string, err error) *FunctionReturnedWithError {
	return &FunctionReturnedWithError{
		inner: &MethodError{
			MethodName: methodName,
			ErrMsg:     err.Error(),
		},
	}
}

// ErrorName returns the error type name.
func (e *FunctionReturnedWithError) ErrorName() string {
	return "FunctionReturnedWithError"
}

// Error returns the error message.
func (e *FunctionReturnedWithError) Error() string {
	return fmt.Sprintf("method `%s` returned with an error: %s", e.inner.MethodName, e.inner.ErrMsg)
}

// TrapOccuredWhileWasmModuleExecutionError indicates that a trap occurred during WASM module execution.
type TrapOccuredWhileWasmModuleExecutionError struct {
	inner *MethodError
}

// ErrorName returns the error type name.
func (e *TrapOccuredWhileWasmModuleExecutionError) ErrorName() string {
	return "TrapOccurredWhileWasmModuleExecution"
}

// Error returns the error message.
func (e *TrapOccuredWhileWasmModuleExecutionError) Error() string {
	return fmt.Sprintf("a trap occurred while executing method `%s`: %s", e.inner.MethodName, e.inner.ErrMsg)
}

// KeyNotFoundInCollectionError indicates that a key was not found in a collection.
type KeyNotFoundInCollectionError struct {
	key string
}

// ErrorName returns the error type name.
func (e *KeyNotFoundInCollectionError) ErrorName() string {
	return "KeyNotFoundInCollection"
}

// Error returns the error message.
func (e *KeyNotFoundInCollectionError) Error() string {
	return fmt.Sprintf("key `%s` not found in the collection state", e.key)
}

// EntriesNotFoundInCollectionForKeysWithPrefixError indicates that no entries were found
// for keys with the given prefix in a collection.
type EntriesNotFoundInCollectionForKeysWithPrefixError struct {
	key string
}

// ErrorName returns the error type name.
func (e *EntriesNotFoundInCollectionForKeysWithPrefixError) ErrorName() string {
	return "EntriesNotFoundInCollectionForKeysWithPrefix"
}

// Error returns the error message.
func (e *EntriesNotFoundInCollectionForKeysWithPrefixError) Error() string {
	return fmt.Sprintf("key prefix `%s` not found in the collection state", e.key)
}

// NoValueReturnedFromDeletingCollectionItemError indicates that no value was returned
// when deleting a collection item (the key was not found).
type NoValueReturnedFromDeletingCollectionItemError struct {
	key string
}

// ErrorName returns the error type name.
func (e *NoValueReturnedFromDeletingCollectionItemError) ErrorName() string {
	return "NoValueReturnedFromDeletingCollectionItem"
}

// Error returns the error message.
func (e *NoValueReturnedFromDeletingCollectionItemError) Error() string {
	return fmt.Sprintf("key `%s` not found in the collection state", e.key)
}

// ContractMethodExecutionError indicates that an error occurred during contract method execution.
type ContractMethodExecutionError struct {
	inner *CrossContractCallError
}

// ErrorName returns the error type name.
func (e *ContractMethodExecutionError) ErrorName() string {
	return "ContractMethodExecutionError"
}

// Error returns the error message.
func (e *ContractMethodExecutionError) Error() string {
	return fmt.Sprintf("error occured while executing contract call with id `%s` to method `%s`: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

// InvalidCrossContractCallError indicates that a cross-contract call was invalid.
type InvalidCrossContractCallError struct {
	inner *CrossContractCallError
}

// ErrorName returns the error type name.
func (e *InvalidCrossContractCallError) ErrorName() string {
	return "InvalidCrossContractCallError"
}

// Error returns the error message.
func (e *InvalidCrossContractCallError) Error() string {
	return fmt.Sprintf("invalid cross contract call with id `%s` to method `%s`: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

// CrossContractCallResultDeserializationError indicates that the result from a cross-contract call
// could not be deserialized.
type CrossContractCallResultDeserializationError struct {
	inner *CrossContractCallError
}

// NewCrossContractCallResultDeserializationError creates a new CrossContractCallResultDeserializationError.
func NewCrossContractCallResultDeserializationError(contractId string, methodName string, errMsg string) *CrossContractCallResultDeserializationError {
	return &CrossContractCallResultDeserializationError{
		inner: &CrossContractCallError{
			ContractId: contractId,
			MethodName: methodName,
			ErrMsg:     errMsg,
		},
	}
}

// ErrorName returns the error type name.
func (e *CrossContractCallResultDeserializationError) ErrorName() string {
	return "CrossContractCallResultDeserializationError"
}

// Error returns the error message.
func (e *CrossContractCallResultDeserializationError) Error() string {
	return fmt.Sprintf("result from cross contract call with id `%s` and method `%s` cannot be deserialized: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

// LLMClusterError indicates that an error occurred in the LLM cluster.
type LLMClusterError struct {
	msg string
}

// ErrorName returns the error type name.
func (e *LLMClusterError) ErrorName() string {
	return "LLMClusterError"
}

// Error returns the error message.
func (e *LLMClusterError) Error() string {
	return fmt.Sprintf("LLM cluster error occured: %s", e.msg)
}

// StreamingResponseDeserializationError indicates that a streaming response could not be deserialized.
type StreamingResponseDeserializationError struct {
	msg string
}

// ErrorName returns the error type name.
func (e *StreamingResponseDeserializationError) ErrorName() string {
	return "StreamingResponseDeserializationError"
}

// Error returns the error message.
func (e *StreamingResponseDeserializationError) Error() string {
	return fmt.Sprintf("streaming response cannot be deserialized: %s", e.msg)
}

// InvalidDataReceivedError indicates that invalid data was received.
type InvalidDataReceivedError struct {
	msg string
}

// ErrorName returns the error type name.
func (e *InvalidDataReceivedError) ErrorName() string {
	return "InvalidDataReceivedError"
}

// Error returns the error message.
func (e *InvalidDataReceivedError) Error() string {
	return fmt.Sprintf("error occurred while reading data: %s", e.msg)
}

// OutcallError indicates that an error occurred while executing an outcall.
type OutcallError struct {
	msg string
}

// ErrorName returns the error type name.
func (e *OutcallError) ErrorName() string {
	return "OutcallError"
}

// Error returns the error message.
func (e *OutcallError) Error() string {
	return fmt.Sprintf("error occured while executing an outcall: %s", e.msg)
}

// InvalidWasmModuleError indicates that an invalid WASM module was encountered.
type InvalidWasmModuleError struct {
	msg string
}

// NewInvalidWasmModuleError creates a new InvalidWasmModuleError.
func NewInvalidWasmModuleError(msg string) *InvalidWasmModuleError {
	return &InvalidWasmModuleError{
		msg: msg,
	}
}

// ErrorName returns the error type name.
func (e *InvalidWasmModuleError) ErrorName() string {
	return "InvalidWasmModuleError"
}

// Error returns the error message.
func (e *InvalidWasmModuleError) Error() string {
	return fmt.Sprintf("error encountered in execution: %s", e.msg)
}
