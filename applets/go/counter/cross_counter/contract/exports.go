package contract

import (
    "github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreFetchCounterFrom() (*uint32, errors.WeilError) {
    type FetchCounterFromArgs struct {
        ContractId string `json:"contract_id"`
    }

    state, args, err := runtime.StateAndArgs[CrossCounterContractState, FetchCounterFromArgs]()

    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("FetchCounterFrom", err)
    }

    result := state.FetchCounterFrom(args.ContractId)

    return &result, nil
}

func FetchCounterFrom() {
    var resp *types.Result[uint32, errors.WeilError]

    result, err := coreFetchCounterFrom()

    if err != nil {
        resp = types.NewErrResult[uint32, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[uint32, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreIncrementCounterOf() *types.Result[runtime.WeilValue[CrossCounterContractState, interface{}], errors.WeilError] {
    type IncrementCounterOfArgs struct {
        ContractId string `json:"contract_id"`
    }

    state, args, err := runtime.StateAndArgs[CrossCounterContractState, IncrementCounterOfArgs]()

    if err != nil {
        var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("IncrementCounterOf", err)
        return types.NewErrResult[runtime.WeilValue[CrossCounterContractState, interface{}], errors.WeilError](&newErr)
    }

    state.IncrementCounterOf(args.ContractId)

    return types.NewOkResult[runtime.WeilValue[CrossCounterContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[CrossCounterContractState, interface{}](state, nil))
}

func IncrementCounterOf() {
    result := coreIncrementCounterOf()

    runtime.SetStateAndResult(result)
}

    func Tools() {
	toolDefs := `[]`

	runtime.SetResult(types.NewOkResult[string, errors.WeilError](&toolDefs))
}
