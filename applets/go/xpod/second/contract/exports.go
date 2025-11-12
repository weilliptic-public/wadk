package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

func coreGetList() (*[]int8, errors.WeilError) {
	type GetListArgs struct {
		Id string `json:"id"`
	}

	state, args, err := runtime.StateAndArgs[SecondContractState, GetListArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("GetList", err)
	}

	result, err := state.GetList(args.Id)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("GetList", err)
	}

	return result, nil
}

func GetList() {
	var resp *types.Result[[]int8, errors.WeilError]

	result, err := coreGetList()

	if err != nil {
		resp = types.NewErrResult[[]int8, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[[]int8, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreSetVal() *types.Result[runtime.WeilValue[SecondContractState, []int8], errors.WeilError] {
	type SetValArgs struct {
		Id  string `json:"id"`
		Val int8   `json:"val"`
	}

	state, args, err := runtime.StateAndArgs[SecondContractState, SetValArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("SetVal", err)
		return types.NewErrResult[runtime.WeilValue[SecondContractState, []int8], errors.WeilError](&newErr)
	}

	result := state.SetVal(args.Id, args.Val)

	return types.NewOkResult[runtime.WeilValue[SecondContractState, []int8], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[SecondContractState, []int8](state, &result))
}

func SetVal() {
	result := coreSetVal()

	runtime.SetStateAndResult(result)
}
