package internal

import (
	"encoding/binary"
	"encoding/json"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal/constants"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"unsafe"
)

func GetWasmPtr[T any](ptr *T) int32 {
	return int32(uintptr(unsafe.Pointer(ptr)))
}

// Memory Layout for WASM memory shared between module and host
// |ERROR ||LENGTH||VALID UTF-8 ENCODED STRING BYTES|
// |1 BYTE||4 BYTE||LENGTH BYTES ...................|

func Read(ptr uintptr) *types.Result[[]byte, errors.WeilError] {
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

func LengthPrefixedBytesFromString(payload []byte, isError byte) []byte {
	len := uint32(len(payload))
	var buffer []byte

	buffer = append(buffer, isError)
	buffer = binary.LittleEndian.AppendUint32(buffer, len)
	buffer = append(buffer, payload...)

	return buffer
}

func LengthPrefixedBytesFromResult[T any](result *types.Result[T, errors.WeilError]) []byte {
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

	return LengthPrefixedBytesFromString(serializedVal, isError)
}
