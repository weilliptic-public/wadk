package main

import (
	"main/contract"

	"github.com/lasarocamargos/jsonmap"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
)

//export __new
func New(len uint, _id uint8) uintptr {
	return runtime.Allocate(len)
}

//export __free
func Free(ptr uintptr, len uint) {
	runtime.Deallocate(ptr, len)
}

//export init
func Init() {
	var resp *types.Result[interface{}, errors.WasmHostInterfaceError]
	state, err := contract.NewCrossCounterContractState()

	if err != nil {
		var newErr errors.WasmHostInterfaceError = errors.NewFunctionReturnedWithError("NewCrossCounterContractState", err)
		resp = types.NewErrResult[interface{}](&newErr)
	} else {
		resp = types.NewOkResult[interface{}, errors.WasmHostInterfaceError](nil)
	}

	runtime.SetState(state)
	runtime.SetResult(resp)
}

//export fetch_counter_from
func FetchCounterFrom() {
	contract.FetchCounterFrom()
}

//export increment_counter_of
func IncrementCounterOf() {
	contract.IncrementCounterOf()
}

//export method_kind_data
func MethodKindData() {
	methodKindMapping := jsonmap.New()

	methodKindMapping.Set("fetch_counter_from", "query")
	methodKindMapping.Set("increment_counter_of", "mutate")

	resp := types.NewOkResult[jsonmap.Map, errors.WasmHostInterfaceError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
