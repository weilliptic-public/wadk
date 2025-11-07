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
	var resp *types.Result[runtime.WeilValue[contract.AsciiArtContractState, interface{}], errors.WeilError]
	state, err := contract.NewAsciiArtContractState()

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("AsciiArtContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	} else {
		resp = types.NewOkResult[runtime.WeilValue[contract.AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.AsciiArtContractState, interface{}](state, nil))
	}

	runtime.SetStateAndResult(resp)
}

//export name
func Name() {
	contract.Name()
}

//export balance_of
func BalanceOf() {
	contract.BalanceOf()
}

//export owner_of
func OwnerOf() {
	contract.OwnerOf()
}

//export details
func Details() {
	contract.Details()
}

//export approve
func Approve() {
	contract.Approve()
}

//export set_approve_for_all
func SetApproveForAll() {
	contract.SetApproveForAll()
}

//export transfer
func Transfer() {
	contract.Transfer()
}

//export transfer_from
func TransferFrom() {
	contract.TransferFrom()
}

//export get_approved
func GetApproved() {
	contract.GetApproved()
}

//export is_approved_for_all
func IsApprovedForAll() {
	contract.IsApprovedForAll()
}

//export mint
func Mint() {
	contract.Mint()
}

//export method_kind_data
func MethodKindData() {
	methodKindMapping := jsonmap.New()

	methodKindMapping.Set("name", "query")
	methodKindMapping.Set("balance_of", "query")
	methodKindMapping.Set("owner_of", "query")
	methodKindMapping.Set("details", "query")
	methodKindMapping.Set("approve", "mutate")
	methodKindMapping.Set("set_approve_for_all", "mutate")
	methodKindMapping.Set("transfer", "mutate")
	methodKindMapping.Set("transfer_from", "mutate")
	methodKindMapping.Set("get_approved", "query")
	methodKindMapping.Set("is_approved_for_all", "query")
	methodKindMapping.Set("mint", "mutate")

	resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
