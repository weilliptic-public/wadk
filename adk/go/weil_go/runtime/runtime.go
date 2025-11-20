package runtime

import (
	"bytes"
	"encoding/json"
	"fmt"
	"runtime"
	"strconv"
	"unsafe"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	internal "github.com/weilliptic-public/wadk/adk/go/weil_go/internal"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

//go:wasmimport env get_state_and_args
func get_state_and_args() int32

//go:wasmimport env set_state_and_result
func set_state_and_result(stateResult int32)

//go:wasmimport env debug_log
func debug_log(log int32)

//go:wasmimport env get_contract_id
func get_contract_id() int32

//go:wasmimport env get_ledger_contract_id
func get_ledger_contract_id() int32

//go:wasmimport env get_sender
func get_sender() int32

//go:wasmimport env get_txn_instantiator_addr
func get_txn_instantiator_addr() int32

//go:wasmimport env get_block_height
func get_block_height() int32

//go:wasmimport env get_block_timestamp
func get_block_timestamp() int32

//go:wasmimport env call_contract
func call_contract(contract int32) int32

//go:wasmimport env call_xpod_contract
func call_xpod_contract(contract int32) int32

//go:wasmimport env write_collection
func write_collection(key int32, val int32)

//go:wasmimport env delete_collection
func delete_collection(key int32) int32

//go:wasmimport env read_collection
func read_collection(key int32) int32

//go:wasmimport env read_bulk_collection
func read_bulk_collection(key int32) int32

var WasmHostMemorySegment []byte // this is to keep data alive!: TODO - find a different way rather than this

type StateArgsValue struct {
	State string `json:"state"`
	Args  string `json:"args"`
}

type StateResultValue struct {
	State *types.Option[string] `json:"state"`
	Value string                `json:"value"`
}

type WeilValue[T any, U any] struct {
	State *types.Option[T] `json:"state"`
	OkVal *U               `json:"ok_val"`
}

func NewWeilValueWithOkValue[T any, U any](val *U) *WeilValue[T, U] {
	return &WeilValue[T, U]{
		State: types.NewNoneOption[T](),
		OkVal: val,
	}
}

func NewWeilValueWithStateAndOkValue[T any, U any](state *T, val *U) *WeilValue[T, U] {
	return &WeilValue[T, U]{
		State: types.NewSomeOption[T](state),
		OkVal: val,
	}
}

func (v *WeilValue[T, U]) raw() *StateResultValue {
	// Create a buffer and encoder that doesn't escape HTML
	var valBuf bytes.Buffer
	valEncoder := json.NewEncoder(&valBuf)
	valEncoder.SetEscapeHTML(false)
	valEncoder.Encode(v.OkVal)
	// Remove the trailing newline
	valBytes := bytes.TrimSpace(valBuf.Bytes())

	if v.State.IsSomeResult() {
		var stateBuf bytes.Buffer
		stateEncoder := json.NewEncoder(&stateBuf)
		stateEncoder.SetEscapeHTML(false)
		stateEncoder.Encode(v.State.TrySomeOption())
		// Remove the trailing newline
		stateBytes := bytes.TrimSpace(stateBuf.Bytes())
		s := string(stateBytes)

		return &StateResultValue{
			State: types.NewSomeOption(&s),
			Value: string(valBytes),
		}
	} else {
		return &StateResultValue{
			State: types.NewNoneOption[string](),
			Value: string(valBytes),
		}
	}
}

func Allocate(len uint) uintptr {
	data := make([]byte, len)
	ptr := uintptr(unsafe.Pointer(&data[0]))
	WasmHostMemorySegment = data
	return ptr
}

func Deallocate(ptr uintptr, len uint) {
	WasmHostMemorySegment = make([]byte, 0) // remove the global reference to the underlying buffer so that it can be collected!
}

// Below functions are the Go-style wrapper functions over raw runtime functions

// Adds/Overwrites an entry to the collection
func WriteCollectionEntry[V any](key []byte, val *V) {
	lfKey := internal.LengthPrefixedBytesFromString(key, 0)
	lfVal := internal.LengthPrefixedBytesFromResult(types.NewOkResult[V, errors.WeilError](val))
	write_collection(internal.GetWasmPtr(&lfKey[0]), internal.GetWasmPtr(&lfVal[0]))

	runtime.KeepAlive(lfKey)
	runtime.KeepAlive(lfVal)
}

// Deletes and returns an entry from the collection.
func DeleteCollectionEntry[V any](key []byte) (*V, error) {
	lfKey := internal.LengthPrefixedBytesFromString(key, 0)
	valPtr := (uintptr)(delete_collection(internal.GetWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := internal.Read(valPtr)

	if result.IsErrResult() {
		errResult := result.TryErrResult()

		switch (*errResult).(type) {
		case *errors.KeyNotFoundInCollectionError:
			return nil, nil
		case *errors.NoValueReturnedFromDeletingCollectionItemError:
			return nil, nil
		default:
			panic(fmt.Sprintf("error other than `KeyNotFoundInCollection` received while `DeleteCollection`: %v", (*errResult).Error()))
		}
	}

	okResult := *result.TryOkResult()
	var val V
	_ = json.Unmarshal(okResult, &val)

	return &val, nil
}

func ReadCollection[V any](key []byte) (*V, error) {
	lfKey := internal.LengthPrefixedBytesFromString(key, 0)
	valPtr := (uintptr)(read_collection(internal.GetWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := internal.Read(valPtr)

	if result.IsErrResult() {
		errResult := result.TryErrResult()

		switch (*errResult).(type) {
		case *errors.KeyNotFoundInCollectionError:
			return nil, *errResult
		default:
			panic(fmt.Sprintf("error other than `KeyNotFoundInCollection` received while `ReadCollection`: %v", (*errResult).Error()))
		}
	}

	okResult := *result.TryOkResult()

	var val V
	_ = json.Unmarshal(okResult, &val)

	return &val, nil
}

func ReadBulkCollection[V any](prefix []byte) (*V, error) {
	lfKey := internal.LengthPrefixedBytesFromString(prefix, 0)
	valPtr := (uintptr)(read_bulk_collection(internal.GetWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := internal.Read(valPtr)

	if result.IsErrResult() {
		errResult := result.TryErrResult()

		switch (*errResult).(type) {
		case *errors.KeyNotFoundInCollectionError:
			return nil, *errResult
		default:
			panic(fmt.Sprintf("error other than `EntriesNotFoundInCollectionForKeysWithPrefix` received while `read_prefix_for_trie`: %v", (*errResult).Error()))
		}
	}

	okResult := *result.TryOkResult()
	var val V
	_ = json.Unmarshal(okResult, &val)

	return &val, nil
}

func State[T any]() *T {
	dataPtr := (uintptr)(get_state_and_args())
	dataJSON := *internal.Read(dataPtr).TryOkResult()

	var stateArgs StateArgsValue

	_ = json.Unmarshal(dataJSON, &stateArgs)

	var state T

	_ = json.Unmarshal([]byte(stateArgs.State), &state)

	return &state
}

func Args[T any]() (*T, error) {
	dataPtr := (uintptr)(get_state_and_args())
	dataJSON := *internal.Read(dataPtr).TryOkResult()

	var stateArgs StateArgsValue

	_ = json.Unmarshal(dataJSON, &stateArgs)

	var args T

	err := json.Unmarshal([]byte(stateArgs.Args), &args)

	if err != nil {
		return nil, err
	} else {
		return &args, nil
	}
}

func StateAndArgs[T any, U any]() (*T, *U, error) {
	dataPtr := (uintptr)(get_state_and_args())
	dataJSON := *internal.Read(dataPtr).TryOkResult()

	var stateArgs StateArgsValue

	_ = json.Unmarshal(dataJSON, &stateArgs)

	var state T
	var args U

	_ = json.Unmarshal([]byte(stateArgs.State), &state)
	err := json.Unmarshal([]byte(stateArgs.Args), &args)

	if err != nil {
		return &state, nil, err
	} else {
		return &state, &args, nil
	}
}

func SetResult[T any](result *types.Result[T, errors.WeilError]) {
	var final_result *types.Result[WeilValue[interface{}, T], errors.WeilError]

	if result.IsOkResult() {
		val := result.TryOkResult()
		weilValue := NewWeilValueWithOkValue[interface{}, T](val)
		final_result = types.NewOkResult[WeilValue[interface{}, T], errors.WeilError](weilValue)
	} else {
		err := result.TryErrResult()
		final_result = types.NewErrResult[WeilValue[interface{}, T], errors.WeilError](err)
	}

	SetStateAndResult(final_result)
}

func SetStateAndResult[T any, U any](result *types.Result[WeilValue[T, U], errors.WeilError]) {
	var final_result *types.Result[StateResultValue, errors.WeilError]

	if result.IsOkResult() {
		val := result.TryOkResult()
		final_result = types.NewOkResult[StateResultValue, errors.WeilError](val.raw())
	} else {
		err := result.TryErrResult()
		final_result = types.NewErrResult[StateResultValue, errors.WeilError](err)
	}

	serializedResult := internal.LengthPrefixedBytesFromResult(final_result)
	set_state_and_result(internal.GetWasmPtr(&serializedResult[0]))
	runtime.KeepAlive(serializedResult)
}

func ContractId() string {
	dataPtr := (uintptr)(get_contract_id())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func LedgerContractId() string {
	dataPtr := (uintptr)(get_ledger_contract_id())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func Sender() string {
	dataPtr := (uintptr)(get_sender())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	var data string = string(dataRaw)
	return data
}

func Initiator() string {
	dataPtr := (uintptr)(get_txn_instantiator_addr())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func BlockHeight() uint32 {
	dataPtr := (uintptr)(get_block_height())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	dataStr := string(dataRaw)
	data, err := strconv.ParseInt(dataStr, 10, 32)
	if err != nil {
		panic("invalid block number")
	}

	return uint32(data)
}

func BlockTimestamp() string {
	dataPtr := (uintptr)(get_block_timestamp())

	dataRaw := *internal.Read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func CallContract[T any](contractId string, methodName string, methodArgs string) (*T, error) {
	type CrossContractCallArgs struct {
		ContractId string `json:"id"`
		MethodName string `json:"method_name"`
		MethodArgs string `json:"method_args"`
	}

	args := CrossContractCallArgs{
		ContractId: contractId,
		MethodName: methodName,
		MethodArgs: methodArgs,
	}

	lfArgs := internal.LengthPrefixedBytesFromResult(types.NewOkResult[CrossContractCallArgs, errors.WeilError](&args))
	resultPtr := (uintptr)(call_contract(internal.GetWasmPtr(&lfArgs[0])))
	runtime.KeepAlive(lfArgs)
	parsedResult := internal.Read(resultPtr)

	if parsedResult.IsErrResult() {
		return nil, *parsedResult.TryErrResult()
	}

	okResult := *parsedResult.TryOkResult()
	var result T
	err := json.Unmarshal(okResult, &result)

	if err != nil {
		return nil, errors.NewCrossContractCallResultDeserializationError(contractId, methodName, err.Error())
	}

	return &result, nil
}

func CallXpodContract(contractId string, methodName string, methodArgs string) (string, error) {
	type CrossContractCallArgs struct {
		ContractId string `json:"id"`
		MethodName string `json:"method_name"`
		MethodArgs string `json:"method_args"`
	}

	args := CrossContractCallArgs{
		ContractId: contractId,
		MethodName: methodName,
		MethodArgs: methodArgs,
	}

	lfArgs := internal.LengthPrefixedBytesFromResult(types.NewOkResult[CrossContractCallArgs, errors.WeilError](&args))
	resultPtr := (uintptr)(call_xpod_contract(internal.GetWasmPtr(&lfArgs[0])))
	runtime.KeepAlive(lfArgs)
	parsedResult := internal.Read(resultPtr)

	if parsedResult.IsErrResult() {
		return "", *parsedResult.TryErrResult()
	}

	okResult := *parsedResult.TryOkResult()
	xpod_id := string(okResult)

	return xpod_id, nil
}

func DebugLog(log string) {
	lfLog := internal.LengthPrefixedBytesFromString([]byte(log), 0)
	debug_log(internal.GetWasmPtr(&lfLog[0]))

	runtime.KeepAlive(lfLog)
}
