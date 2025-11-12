package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
    "github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreFetchCounterFrom() (*uint32, errors.WasmHostInterfaceError) {
    state, _ := runtime.State[CrossCounterContractState]()

    type FetchCounterFromArgs struct {
        ContractId string `json:"contract_id"`
    }
    
    args, err := runtime.Args[FetchCounterFromArgs]()
    
    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("FetchCounterFrom", err)
    }

    result := state.FetchCounterFrom(args.ContractId)

    return &result, nil
}

func FetchCounterFrom() {
    var resp *types.Result[uint32, errors.WasmHostInterfaceError]

    result, err := coreFetchCounterFrom()


    if err != nil {
		resp = types.NewErrResult[uint32, errors.WasmHostInterfaceError](&err)
    } else {
		resp = types.NewOkResult[uint32, errors.WasmHostInterfaceError](result)
    }

    runtime.SetResult(resp)
}

func coreIncrementCounterOf() errors.WasmHostInterfaceError {
    state, _ := runtime.State[CrossCounterContractState]()

    type IncrementCounterOfArgs struct {
        ContractId string `json:"contract_id"`
    }
    
    args, err := runtime.Args[IncrementCounterOfArgs]()
    
    if err != nil {
        return errors.NewMethodArgumentDeserializationError("IncrementCounterOf", err)
    }

    state.IncrementCounterOf(args.ContractId)

    runtime.SetState(state)

    return nil
}

func IncrementCounterOf() {
    var resp *types.Result[interface{}, errors.WasmHostInterfaceError]

    err := coreIncrementCounterOf()


    if err != nil {
		resp = types.NewErrResult[interface{}, errors.WasmHostInterfaceError](&err)
    } else {
		resp = types.NewOkResult[interface{}, errors.WasmHostInterfaceError](nil)
    }

    runtime.SetResult(resp)
}
