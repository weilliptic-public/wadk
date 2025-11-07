package contract

import (
    "github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreGetCount() (*uint32, errors.WeilError) {
    state := runtime.State[CounterContractState]()
    result := state.GetCount()

    return &result, nil
}

func GetCount() {
    var resp *types.Result[uint32, errors.WeilError]

    result, err := coreGetCount()

    if err != nil {
        resp = types.NewErrResult[uint32, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[uint32, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreIncrement() *types.Result[runtime.WeilValue[CounterContractState, interface{}], errors.WeilError] {
    state := runtime.State[CounterContractState]()
    state.Increment()

    return types.NewOkResult[runtime.WeilValue[CounterContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[CounterContractState, interface{}](state, nil))
}

func Increment() {
    result := coreIncrement()

    runtime.SetStateAndResult(result)
}

func coreSetValue() *types.Result[runtime.WeilValue[CounterContractState, interface{}], errors.WeilError] {
    type SetValueArgs struct {
        Val uint32 `json:"val"`
    }

    state, args, err := runtime.StateAndArgs[CounterContractState, SetValueArgs]()

    if err != nil {
        var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("SetValue", err)
        return types.NewErrResult[runtime.WeilValue[CounterContractState, interface{}], errors.WeilError](&newErr)
    }

    state.SetValue(args.Val)

    return types.NewOkResult[runtime.WeilValue[CounterContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[CounterContractState, interface{}](state, nil))
}

func SetValue() {
    result := coreSetValue()

    runtime.SetStateAndResult(result)
}
