package errors

import (
	"encoding/json"
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal/constants"
)

type WeilError interface {
	ErrorName() string
	error
}

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

type MethodError struct {
	MethodName string `json:"method_name"`
	ErrMsg     string `json:"err_msg"`
}

type CrossContractCallError struct {
	ContractId string `json:"contract_id"`
	MethodName string `json:"method_name"`
	ErrMsg     string `json:"err_msg"`
}

type MethodArgumentDeserializationError struct {
	inner *MethodError
}

func NewMethodArgumentDeserializationError(methodName string, err error) *MethodArgumentDeserializationError {
	return &MethodArgumentDeserializationError{
		inner: &MethodError{
			MethodName: methodName,
			ErrMsg:     err.Error(),
		},
	}
}

func (e *MethodArgumentDeserializationError) ErrorName() string {
	return "MethodArgumentDeserializationError"
}

func (e *MethodArgumentDeserializationError) Error() string {
	return fmt.Sprintf("arguments for method `%s` cannot be deserialized: `%s`", e.inner.MethodName, e.inner.ErrMsg)
}

type FunctionReturnedWithError struct {
	inner *MethodError
}

func NewFunctionReturnedWithError(methodName string, err error) *FunctionReturnedWithError {
	return &FunctionReturnedWithError{
		inner: &MethodError{
			MethodName: methodName,
			ErrMsg:     err.Error(),
		},
	}
}

func (e *FunctionReturnedWithError) ErrorName() string {
	return "FunctionReturnedWithError"
}

func (e *FunctionReturnedWithError) Error() string {
	return fmt.Sprintf("method `%s` returned with an error: %s", e.inner.MethodName, e.inner.ErrMsg)
}

type TrapOccuredWhileWasmModuleExecutionError struct {
	inner *MethodError
}

func (e *TrapOccuredWhileWasmModuleExecutionError) ErrorName() string {
	return "TrapOccurredWhileWasmModuleExecution"
}

func (e *TrapOccuredWhileWasmModuleExecutionError) Error() string {
	return fmt.Sprintf("a trap occurred while executing method `%s`: %s", e.inner.MethodName, e.inner.ErrMsg)
}

type KeyNotFoundInCollectionError struct {
	key string
}

func (e *KeyNotFoundInCollectionError) ErrorName() string {
	return "KeyNotFoundInCollection"
}

func (e *KeyNotFoundInCollectionError) Error() string {
	return fmt.Sprintf("key `%s` not found in the collection state", e.key)
}

type EntriesNotFoundInCollectionForKeysWithPrefixError struct {
	key string
}

func (e *EntriesNotFoundInCollectionForKeysWithPrefixError) ErrorName() string {
	return "EntriesNotFoundInCollectionForKeysWithPrefix"
}

func (e *EntriesNotFoundInCollectionForKeysWithPrefixError) Error() string {
	return fmt.Sprintf("key prefix `%s` not found in the collection state", e.key)
}

type NoValueReturnedFromDeletingCollectionItemError struct {
	key string
}

func (e *NoValueReturnedFromDeletingCollectionItemError) ErrorName() string {
	return "NoValueReturnedFromDeletingCollectionItem"
}

func (e *NoValueReturnedFromDeletingCollectionItemError) Error() string {
	return fmt.Sprintf("key `%s` not found in the collection state", e.key)
}

type ContractMethodExecutionError struct {
	inner *CrossContractCallError
}

func (e *ContractMethodExecutionError) ErrorName() string {
	return "ContractMethodExecutionError"
}

func (e *ContractMethodExecutionError) Error() string {
	return fmt.Sprintf("error occured while executing contract call with id `%s` to method `%s`: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

type InvalidCrossContractCallError struct {
	inner *CrossContractCallError
}

func (e *InvalidCrossContractCallError) ErrorName() string {
	return "InvalidCrossContractCallError"
}

func (e *InvalidCrossContractCallError) Error() string {
	return fmt.Sprintf("invalid cross contract call with id `%s` to method `%s`: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

type CrossContractCallResultDeserializationError struct {
	inner *CrossContractCallError
}

func NewCrossContractCallResultDeserializationError(contractId string, methodName string, errMsg string) *CrossContractCallResultDeserializationError {
	return &CrossContractCallResultDeserializationError{
		inner: &CrossContractCallError{
			ContractId: contractId,
			MethodName: methodName,
			ErrMsg:     errMsg,
		},
	}
}

func (e *CrossContractCallResultDeserializationError) ErrorName() string {
	return "CrossContractCallResultDeserializationError"
}

func (e *CrossContractCallResultDeserializationError) Error() string {
	return fmt.Sprintf("result from cross contract call with id `%s` and method `%s` cannot be deserialized: %s", e.inner.ContractId, e.inner.MethodName, e.inner.ErrMsg)
}

type LLMClusterError struct {
	msg string
}

func (e *LLMClusterError) ErrorName() string {
	return "LLMClusterError"
}

func (e *LLMClusterError) Error() string {
	return fmt.Sprintf("LLM cluster error occured: %s", e.msg)
}

type StreamingResponseDeserializationError struct {
	msg string
}

func (e *StreamingResponseDeserializationError) ErrorName() string {
	return "StreamingResponseDeserializationError"
}

func (e *StreamingResponseDeserializationError) Error() string {
	return fmt.Sprintf("streaming response cannot be deserialized: %s", e.msg)
}

type InvalidDataReceivedError struct {
	msg string
}

func (e *InvalidDataReceivedError) ErrorName() string {
	return "InvalidDataReceivedError"
}

func (e *InvalidDataReceivedError) Error() string {
	return fmt.Sprintf("error occurred while reading data: %s", e.msg)
}

type OutcallError struct {
	msg string
}

func NewOutcallError(msg string) *OutcallError {
	return &OutcallError{
		msg: msg,
	}
}

func (e *OutcallError) ErrorName() string {
	return "OutcallError"
}

func (e *OutcallError) Error() string {
	return fmt.Sprintf("error occured while executing an outcall: %s", e.msg)
}

type InvalidWasmModuleError struct {
	msg string
}

func NewInvalidWasmModuleError(msg string) *InvalidWasmModuleError {
	return &InvalidWasmModuleError{
		msg: msg,
	}
}

func (e *InvalidWasmModuleError) ErrorName() string {
	return "InvalidWasmModuleError"
}

func (e *InvalidWasmModuleError) Error() string {
	return fmt.Sprintf("error encountered in execution: %s", e.msg)
}
