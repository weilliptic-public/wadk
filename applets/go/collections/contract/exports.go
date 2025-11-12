package contract

import (
    "github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreGet() errors.WasmHostInterfaceError {
    runtime.DebugLog("coreGet1")
    state, err := runtime.State[CollectionsContractState]()
    runtime.DebugLog("coreGet2")

    if err != nil {
        state = NewCollectionsContractState()
    }

    state.Get()

    return nil
}

func Get() {
    var resp *types.Result[string, errors.WasmHostInterfaceError]
    runtime.DebugLog("Get1")

    err := coreGet()

    runtime.DebugLog("Get2")


    if err != nil {
		resp = types.NewErrResult[string, errors.WasmHostInterfaceError](&err)
    } else {
		resp = types.NewOkResult[string, errors.WasmHostInterfaceError](nil)
    }

    runtime.SetResult(resp)
}

func coreSet() errors.WasmHostInterfaceError {
    state, err := runtime.State[CollectionsContractState]()

    if err != nil {
        state = NewCollectionsContractState()
    }

    state.Set()

    runtime.SetState(state)

    return nil
}

func Set() {
    var resp *types.Result[string, errors.WasmHostInterfaceError]

    err := coreSet()


    if err != nil {
		resp = types.NewErrResult[string, errors.WasmHostInterfaceError](&err)
    } else {
		resp = types.NewOkResult[string, errors.WasmHostInterfaceError](nil)
    }

    runtime.SetResult(resp)
}
