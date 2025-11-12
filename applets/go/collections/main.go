package main

import (
	"main/contract"

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
	state := contract.NewCollectionsContractState()
	runtime.SetState(state)

	resp := types.NewOkResult[string, errors.WasmHostInterfaceError](nil)
	runtime.SetResult(resp)
}

//export get
func Get() {
	contract.Get()
}

//export set
func Set() {
	contract.Set()
}

func main() {}
