package contract

import (
    "github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreGet() errors.WeilError {
    state := runtime.State[CollectionsContractState]()
    state.Get()

    return nil
}

func Get() {
    var resp *types.Result[interface{}, errors.WeilError]

    err := coreGet()

    if err != nil {
        resp = types.NewErrResult[interface{}, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[interface{}, errors.WeilError](nil)
    }

    runtime.SetResult(resp)
}

func coreSet() *types.Result[runtime.WeilValue[CollectionsContractState, interface{}], errors.WeilError] {
    state := runtime.State[CollectionsContractState]()
    state.Set()

    return types.NewOkResult[runtime.WeilValue[CollectionsContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[CollectionsContractState, interface{}](state, nil))
}

func Set() {
    result := coreSet()

    runtime.SetStateAndResult(result)
}

    func Tools() {
	toolDefs := `[]`

	runtime.SetResult(types.NewOkResult[string, errors.WeilError](&toolDefs))
}
