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

//export init
func Init() {
	var resp *types.Result[runtime.WeilValue[contract.CounterContractState, interface{}], errors.WeilError]
	state, err := contract.NewCounterContractState()

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("CounterContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.CounterContractState, interface{}], errors.WeilError](&newErr)
	} else {
		resp = types.NewOkResult[runtime.WeilValue[contract.CounterContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.CounterContractState, interface{}](state, nil))
	}

	runtime.SetStateAndResult(resp)
}

//export get_count
func GetCount() {
	contract.GetCount()
}

//export increment
func Increment() {
	contract.Increment()
}

//export set_value
func SetValue() {
	contract.SetValue()
}

//export method_kind_data
func MethodKindData() {
	methodKindMapping := jsonmap.New()

	methodKindMapping.Set("get_count", "query")
	methodKindMapping.Set("increment", "mutate")
	methodKindMapping.Set("set_value", "mutate")

	resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
