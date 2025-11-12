package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/utils"
)

func coreHealthCheck() (*string, errors.WeilError) {
	state := runtime.State[FirstContractState]()
	result := state.HealthCheck()

	return &result, nil
}

func HealthCheck() {
	var resp *types.Result[string, errors.WeilError]

	result, err := coreHealthCheck()

	if err != nil {
		resp = types.NewErrResult[string, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[string, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreCounter() (*uint32, errors.WeilError) {
	type CounterArgs struct {
		Id string `json:"id"`
	}

	state, args, err := runtime.StateAndArgs[FirstContractState, CounterArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("Counter", err)
	}

	result, err := state.Counter(args.Id)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("Counter", err)
	}

	return result, nil
}

func Counter() {
	var resp *types.Result[uint32, errors.WeilError]

	result, err := coreCounter()

	if err != nil {
		resp = types.NewErrResult[uint32, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint32, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreSetListInSecond() *types.Result[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError] {
	type SetListInSecondArgs struct {
		ContractId string `json:"contract_id"`
		Id         string `json:"id"`
		Val        int8   `json:"val"`
	}

	state, args, err := runtime.StateAndArgs[FirstContractState, SetListInSecondArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("SetListInSecond", err)
		return types.NewErrResult[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.SetListInSecond(args.ContractId, args.Id, args.Val)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("SetListInSecond", err)
		return types.NewErrResult[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[FirstContractState, interface{}](state, nil))
}

func SetListInSecond() {
	result := coreSetListInSecond()

	runtime.SetStateAndResult(result)
}

func coreSetListInSecondCallback() *types.Result[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError] {

	type CallbackArgs struct {
		Result *types.Result[string, errors.WeilError] `json:"result"`
		XpodId string                                  `json:"xpod_id"`
	}

	state, args, err := runtime.StateAndArgs[FirstContractState, CallbackArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("SetListInSecondCallback", err)
		return types.NewErrResult[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError](&newErr)
	}

	xpodId := args.XpodId

	args1 := utils.TryIntoResult[[]int8](args.Result)
	type Args struct {
		Result *types.Result[[]int8, string] `json:"result"`
		XpodId string                        `json:"xpod_id"`
	}

	args2 := Args{Result: args1, XpodId: xpodId}
	state.SetListInSecondCallback(args2.XpodId, *args2.Result)

	return types.NewOkResult[runtime.WeilValue[FirstContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[FirstContractState, interface{}](state, nil))
}

func SetListInSecondCallback() {
	result := coreSetListInSecondCallback()

	runtime.SetStateAndResult(result)
}
