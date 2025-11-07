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
	var resp *types.Result[runtime.WeilValue[contract.YutakaContractState, interface{}], errors.WeilError]
	state, err := contract.NewYutakaContractState()

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("NewYutakaContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.YutakaContractState, interface{}], errors.WeilError](&newErr)
	} else {
		resp = types.NewOkResult[runtime.WeilValue[contract.YutakaContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.YutakaContractState, interface{}](state, nil))
	}

	runtime.SetStateAndResult(resp)
}

//export name
func Name() {
	contract.Name()
}

//export symbol
func Symbol() {
	contract.Symbol()
}

//export decimals
func Decimals() {
	contract.Decimals()
}

//export details
func Details() {
	contract.Details()
}

//export total_supply
func TotalSupply() {
	contract.TotalSupply()
}

//export balance_for
func BalanceFor() {
	contract.BalanceFor()
}

//export transfer
func Transfer() {
	contract.Transfer()
}

//export approve
func Approve() {
	contract.Approve()
}

//export transfer_from
func TransferFrom() {
	contract.TransferFrom()
}

//export allowance
func Allowance() {
	contract.Allowance()
}

//export method_kind_data
func MethodKindData() {
	methodKindMapping := jsonmap.New()

	methodKindMapping.Set("name", "query")
	methodKindMapping.Set("symbol", "query")
	methodKindMapping.Set("decimals", "query")
	methodKindMapping.Set("details", "query")
	methodKindMapping.Set("total_supply", "query")
	methodKindMapping.Set("balance_for", "query")
	methodKindMapping.Set("transfer", "mutate")
	methodKindMapping.Set("approve", "mutate")
	methodKindMapping.Set("transfer_from", "mutate")
	methodKindMapping.Set("allowance", "query")

	resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
