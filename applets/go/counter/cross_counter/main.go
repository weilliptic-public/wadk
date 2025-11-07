package main

import (
	"main/contract"

	"github.com/weilliptic-public/jsonmap"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

//export __new
func New(len uint, _id uint8) uintptr {
	return runtime.Allocate(len)
}

//export __free
func Free(ptr uintptr, len uint) {
	runtime.Deallocate(ptr, len)
}

//export tools
func Tools() {
	contract.Tools()
}

//export init
func Init() {
    var resp *types.Result[runtime.WeilValue[contract.CrossCounterContractState, interface{}], errors.WeilError]
	state, err := contract.NewCrossCounterContractState()

    if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("NewCrossCounterContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.CrossCounterContractState, interface{}], errors.WeilError](&newErr)
    } else {
		resp = types.NewOkResult[runtime.WeilValue[contract.CrossCounterContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.CrossCounterContractState, interface{}](state, nil))
    }

	runtime.SetStateAndResult(resp)
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


    resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
