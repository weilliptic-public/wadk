package runtime

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"runtime"
	"strconv"
	"unsafe"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/internal/constants"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
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

func getWasmPtr[T any](ptr *T) int32 {
	return int32(uintptr(unsafe.Pointer(ptr)))
}

// Memory Layout for WASM memory shared between module and host
// |ERROR ||LENGTH||VALID UTF-8 ENCODED STRING BYTES|
// |1 BYTE||4 BYTE||LENGTH BYTES ...................|

func read(ptr uintptr) *types.Result[[]byte, errors.WeilError] {
	// TODO - reimplement it such that instead of extra allocation of buffer
	// we take the ownership of the existing buffer from it's pointer and len.
	lenBuffer := make([]byte, constants.WASM_MEMORY_SEGMENT_LENGTH_BYTES_SIZE)

	isError := *(*int8)(unsafe.Pointer(ptr))
	for i := 0; i < 4; i++ {
		lenBuffer[i] = *(*byte)(unsafe.Pointer(ptr + 1 + uintptr(i)))
	}

	length := binary.LittleEndian.Uint32(lenBuffer)
	buf := make([]byte, length)

	for i := 0; i < int(length); i++ {
		buf[i] = *(*byte)(unsafe.Pointer(ptr + 1 + 4 + uintptr(i)))
	}

	switch isError {
	case -1:
		var err errors.WeilError = errors.NewInvalidWasmModuleError("WASM Size Limit Reached")
		return types.NewErrResult[[]byte, errors.WeilError](&err)

	case -2:
		var err errors.WeilError = errors.NewInvalidWasmModuleError("invalid __new function export in module")
		return types.NewErrResult[[]byte, errors.WeilError](&err)

	case -3:
		var err errors.WeilError = errors.NewInvalidWasmModuleError("invalid __free function export in module")
		return types.NewErrResult[[]byte, errors.WeilError](&err)

	case 0:
		return types.NewOkResult[[]byte, errors.WeilError](&buf)

	case 1:
		err := errors.WasmHostInterfaceErrorFromBytes(buf)
		return types.NewErrResult[[]byte, errors.WeilError](&err)

	default:
		panic(constants.UNREACHABLE)
	}
}

func lengthPrefixedBytesFromString(payload []byte, isError byte) []byte {
	len := uint32(len(payload))
	var buffer []byte

	buffer = append(buffer, isError)
	buffer = binary.LittleEndian.AppendUint32(buffer, len)
	buffer = append(buffer, payload...)

	return buffer
}

func lengthPrefixedBytesFromResult[T any](result *types.Result[T, errors.WeilError]) []byte {
	var serializedVal []byte
	var isError uint8

	if result.IsOkResult() {
		val := result.TryOkResult()
		serializedVal, _ = json.Marshal(val)
		
		isError = 0
	} else {
		serializedVal = errors.MarshalJSONWasmHostInterfaceError(*result.TryErrResult())
		isError = 1
	}

	return lengthPrefixedBytesFromString(serializedVal, isError)
}

// Below functions are the Go-style wrapper functions over raw runtime functions

// Adds/Overwrites an entry to the collection
func WriteCollectionEntry[V any](key []byte, val *V) {
	lfKey := lengthPrefixedBytesFromString(key, 0)
	lfVal := lengthPrefixedBytesFromResult(types.NewOkResult[V, errors.WeilError](val))
	write_collection(getWasmPtr(&lfKey[0]), getWasmPtr(&lfVal[0]))

	runtime.KeepAlive(lfKey)
	runtime.KeepAlive(lfVal)
}

// Deletes and returns an entry from the collection.
func DeleteCollectionEntry[V any](key []byte) (*V, error) {
	lfKey := lengthPrefixedBytesFromString(key, 0)
	valPtr := (uintptr)(delete_collection(getWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := read(valPtr)

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
	lfKey := lengthPrefixedBytesFromString(key, 0)
	valPtr := (uintptr)(read_collection(getWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := read(valPtr)

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
	lfKey := lengthPrefixedBytesFromString(prefix, 0)
	valPtr := (uintptr)(read_bulk_collection(getWasmPtr(&lfKey[0])))
	runtime.KeepAlive(lfKey)
	result := read(valPtr)

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
	dataJSON := *read(dataPtr).TryOkResult()

	var stateArgs StateArgsValue

	_ = json.Unmarshal(dataJSON, &stateArgs)

	var state T

	_ = json.Unmarshal([]byte(stateArgs.State), &state)

	return &state
}

func Args[T any]() (*T, error) {
	dataPtr := (uintptr)(get_state_and_args())
	dataJSON := *read(dataPtr).TryOkResult()

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
	dataJSON := *read(dataPtr).TryOkResult()

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

	serializedResult := lengthPrefixedBytesFromResult(final_result)
	set_state_and_result(getWasmPtr(&serializedResult[0]))
	runtime.KeepAlive(serializedResult)
}

func ContractId() string {
	dataPtr := (uintptr)(get_contract_id())

	dataRaw := *read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func LedgerContractId() string {
	dataPtr := (uintptr)(get_ledger_contract_id())

	dataRaw := *read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func Sender() string {
	dataPtr := (uintptr)(get_sender())

	dataRaw := *read(dataPtr).TryOkResult()
	var data string = string(dataRaw)
	return data
}

func Initiator() string {
	dataPtr := (uintptr)(get_txn_instantiator_addr())

	dataRaw := *read(dataPtr).TryOkResult()
	var data string = string(dataRaw)

	return data
}

func BlockHeight() uint32 {
	dataPtr := (uintptr)(get_block_height())

	dataRaw := *read(dataPtr).TryOkResult()
	dataStr := string(dataRaw)
	data, err := strconv.ParseInt(dataStr, 10, 32)
	if err != nil {
		panic("invalid block number")
	}

	return uint32(data)
}

func BlockTimestamp() string {
	dataPtr := (uintptr)(get_block_timestamp())

	dataRaw := *read(dataPtr).TryOkResult()
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

	lfArgs := lengthPrefixedBytesFromResult(types.NewOkResult[CrossContractCallArgs, errors.WeilError](&args))
	resultPtr := (uintptr)(call_contract(getWasmPtr(&lfArgs[0])))
	runtime.KeepAlive(lfArgs)
	parsedResult := read(resultPtr)

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

	lfArgs := lengthPrefixedBytesFromResult(types.NewOkResult[CrossContractCallArgs, errors.WeilError](&args))
	resultPtr := (uintptr)(call_xpod_contract(getWasmPtr(&lfArgs[0])))
	runtime.KeepAlive(lfArgs)
	parsedResult := read(resultPtr)

	if parsedResult.IsErrResult() {
		return "", *parsedResult.TryErrResult()
	}

	okResult := *parsedResult.TryOkResult()
	xpod_id := string(okResult)

	return xpod_id, nil
}

func DebugLog(log string) {
	lfLog := lengthPrefixedBytesFromString([]byte(log), 0)
	debug_log(getWasmPtr(&lfLog[0]))

	runtime.KeepAlive(lfLog)
}
