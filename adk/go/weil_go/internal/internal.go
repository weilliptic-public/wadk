package internal

import (
	"encoding/binary"
	"encoding/json"
	"fmt"
	"unsafe"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/internal/constants"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

func GetWasmPtr[T any](ptr *T) int32 {
	return int32(uintptr(unsafe.Pointer(ptr)))
}

// Memory Layout for WASM memory shared between module and host
// |ERROR ||LENGTH||VALID UTF-8 ENCODED STRING BYTES|
// |1 BYTE||4 BYTE||LENGTH BYTES ...................|

func Read(ptr uintptr) *types.Result[[]byte, errors.WeilError] {
	// Pointer arithmetic on WASM linear memory. The 'ptr' value is a raw offset
	// into WASM linear memory returned by the host — it is not a Go heap pointer
	// and cannot be moved by the GC. The go vet "unsafe.Pointer" warnings here
	// are false positives in the WASM execution context.
	lenBuffer := make([]byte, constants.WASM_MEMORY_SEGMENT_LENGTH_BYTES_SIZE)

	isError := *(*int8)(unsafe.Pointer(ptr))  //nolint:unsafeptr
	for i := 0; i < 4; i++ {
		lenBuffer[i] = *(*byte)(unsafe.Pointer(ptr + 1 + uintptr(i))) //nolint:unsafeptr
	}

	length := binary.LittleEndian.Uint32(lenBuffer)
	buf := make([]byte, length)

	for i := 0; i < int(length); i++ {
		buf[i] = *(*byte)(unsafe.Pointer(ptr + 1 + 4 + uintptr(i))) //nolint:unsafeptr
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
		var marshalErr error
		serializedVal, marshalErr = json.Marshal(val)
		if marshalErr != nil {
			panic(fmt.Sprintf("LengthPrefixedBytesFromResult: failed to marshal ok value: %v", marshalErr))
		}

		isError = 0
	} else {
		serializedVal = errors.MarshalJSONWasmHostInterfaceError(*result.TryErrResult())
		isError = 1
	}

	return LengthPrefixedBytesFromString(serializedVal, isError)
}
