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
	var resp *types.Result[runtime.WeilValue[contract.SecondContractState, interface{}], errors.WeilError]
	state, err := contract.NewSecondContractState()

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("NewSecondContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.SecondContractState, interface{}], errors.WeilError](&newErr)
	} else {
		resp = types.NewOkResult[runtime.WeilValue[contract.SecondContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.SecondContractState, interface{}](state, nil))
	}

	runtime.SetStateAndResult(resp)
}

//export get_list
func GetList() {
	contract.GetList()
}

//export set_val
func SetVal() {
	contract.SetVal()
}

//export method_kind_data
func MethodKindData() {
	methodKindMapping := jsonmap.New()

	methodKindMapping.Set("get_list", "query")
	methodKindMapping.Set("set_val", "mutate")

	resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
