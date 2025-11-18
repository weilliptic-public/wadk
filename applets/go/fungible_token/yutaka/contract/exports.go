package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

func coreName() (*string, errors.WeilError) {
	state := runtime.State[YutakaContractState]()
	result := state.Name()

	return &result, nil
}

func Name() {
	var resp *types.Result[string, errors.WeilError]

	result, err := coreName()

	if err != nil {
		resp = types.NewErrResult[string, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[string, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreSymbol() (*string, errors.WeilError) {
	state := runtime.State[YutakaContractState]()
	result := state.Symbol()

	return &result, nil
}

func Symbol() {
	var resp *types.Result[string, errors.WeilError]

	result, err := coreSymbol()

	if err != nil {
		resp = types.NewErrResult[string, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[string, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreDecimals() (*uint8, errors.WeilError) {
	state := runtime.State[YutakaContractState]()
	result := state.Decimals()

	return &result, nil
}

func Decimals() {
	var resp *types.Result[uint8, errors.WeilError]

	result, err := coreDecimals()

	if err != nil {
		resp = types.NewErrResult[uint8, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint8, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreDetails() (**types.Tuple3[string, string, uint8], errors.WeilError) {
	state := runtime.State[YutakaContractState]()
	result := state.Details()

	return &result, nil
}

func Details() {
	var resp *types.Result[*types.Tuple3[string, string, uint8], errors.WeilError]

	result, err := coreDetails()

	if err != nil {
		resp = types.NewErrResult[*types.Tuple3[string, string, uint8], errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[*types.Tuple3[string, string, uint8], errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreTotalSupply() (*uint64, errors.WeilError) {
	state := runtime.State[YutakaContractState]()
	result := state.TotalSupply()

	return &result, nil
}

func TotalSupply() {
	var resp *types.Result[uint64, errors.WeilError]

	result, err := coreTotalSupply()

	if err != nil {
		resp = types.NewErrResult[uint64, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint64, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreBalanceFor() (*uint64, errors.WeilError) {
	type BalanceForArgs struct {
		Addr string `json:"addr"`
	}

	state, args, err := runtime.StateAndArgs[YutakaContractState, BalanceForArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("BalanceFor", err)
	}

	result, err := state.BalanceFor(args.Addr)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("BalanceFor", err)
	}

	return result, nil
}

func BalanceFor() {
	var resp *types.Result[uint64, errors.WeilError]

	result, err := coreBalanceFor()

	if err != nil {
		resp = types.NewErrResult[uint64, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint64, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreTransfer() *types.Result[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError] {
	type TransferArgs struct {
		ToAddr string `json:"to_addr"`
		Amount uint64 `json:"amount"`
	}

	state, args, err := runtime.StateAndArgs[YutakaContractState, TransferArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("Transfer", err)
		return types.NewErrResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.Transfer(args.ToAddr, args.Amount)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("Transfer", err)
		return types.NewErrResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[YutakaContractState, interface{}](state, nil))
}

func Transfer() {
	result := coreTransfer()

	runtime.SetStateAndResult(result)
}

func coreApprove() *types.Result[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError] {
	type ApproveArgs struct {
		Spender string `json:"spender"`
		Amount  uint64 `json:"amount"`
	}

	state, args, err := runtime.StateAndArgs[YutakaContractState, ApproveArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("Approve", err)
		return types.NewErrResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](&newErr)
	}

	state.Approve(args.Spender, args.Amount)

	return types.NewOkResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[YutakaContractState, interface{}](state, nil))
}

func Approve() {
	result := coreApprove()

	runtime.SetStateAndResult(result)
}

func coreTransferFrom() *types.Result[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError] {
	type TransferFromArgs struct {
		FromAddr string `json:"from_addr"`
		ToAddr   string `json:"to_addr"`
		Amount   uint64 `json:"amount"`
	}

	state, args, err := runtime.StateAndArgs[YutakaContractState, TransferFromArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("TransferFrom", err)
		return types.NewErrResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.TransferFrom(args.FromAddr, args.ToAddr, args.Amount)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("TransferFrom", err)
		return types.NewErrResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[YutakaContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[YutakaContractState, interface{}](state, nil))
}

func TransferFrom() {
	result := coreTransferFrom()

	runtime.SetStateAndResult(result)
}

func coreAllowance() (*uint64, errors.WeilError) {
	type AllowanceArgs struct {
		Owner   string `json:"owner"`
		Spender string `json:"spender"`
	}

	state, args, err := runtime.StateAndArgs[YutakaContractState, AllowanceArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("Allowance", err)
	}

	result := state.Allowance(args.Owner, args.Spender)

	return &result, nil
}

func Allowance() {
	var resp *types.Result[uint64, errors.WeilError]

	result, err := coreAllowance()

	if err != nil {
		resp = types.NewErrResult[uint64, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint64, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}
