package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

func coreName() (*string, errors.WeilError) {
	state := runtime.State[AsciiArtContractState]()
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

func coreBalanceOf() (*uint64, errors.WeilError) {
	type BalanceOfArgs struct {
		Addr string `json:"addr"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, BalanceOfArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("balance_of", err)
	}

	result := state.BalanceOf(args.Addr)

	return &result, nil
}

func BalanceOf() {
	var resp *types.Result[uint64, errors.WeilError]

	result, err := coreBalanceOf()

	if err != nil {
		resp = types.NewErrResult[uint64, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[uint64, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreOwnerOf() (*string, errors.WeilError) {
	type OwnerOfArgs struct {
		TokenId string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, OwnerOfArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("owner_of", err)
	}

	result, err := state.OwnerOf(args.TokenId)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("owner_of", err)
	}

	return result, nil
}

func OwnerOf() {
	var resp *types.Result[string, errors.WeilError]

	result, err := coreOwnerOf()

	if err != nil {
		resp = types.NewErrResult[string, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[string, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreDetails() (*TokenDetails, errors.WeilError) {
	type DetailsArgs struct {
		TokenId string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, DetailsArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("details", err)
	}

	result, err := state.Details(args.TokenId)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("details", err)
	}

	return result, nil
}

func Details() {
	var resp *types.Result[TokenDetails, errors.WeilError]

	result, err := coreDetails()

	if err != nil {
		resp = types.NewErrResult[TokenDetails, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[TokenDetails, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreApprove() *types.Result[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError] {
	type ApproveArgs struct {
		Spender string `json:"spender"`
		TokenId string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, ApproveArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("approve", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.Approve(args.Spender, args.TokenId)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("approve", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[AsciiArtContractState, interface{}](state, nil))
}

func Approve() {
	result := coreApprove()

	runtime.SetStateAndResult(result)
}

func coreSetApproveForAll() *types.Result[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError] {
	type SetApproveForAllArgs struct {
		Spender  string `json:"spender"`
		Approval bool   `json:"approval"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, SetApproveForAllArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("set_approve_for_all", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	state.SetApproveForAll(args.Spender, args.Approval)

	return types.NewOkResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[AsciiArtContractState, interface{}](state, nil))
}

func SetApproveForAll() {
	result := coreSetApproveForAll()

	runtime.SetStateAndResult(result)
}

func coreTransfer() *types.Result[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError] {
	type TransferArgs struct {
		ToAddr  string `json:"to_addr"`
		TokenId string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, TransferArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("transfer", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.Transfer(args.ToAddr, args.TokenId)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("transfer", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[AsciiArtContractState, interface{}](state, nil))
}

func Transfer() {
	result := coreTransfer()

	runtime.SetStateAndResult(result)
}

func coreTransferFrom() *types.Result[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError] {
	type TransferFromArgs struct {
		FromAddr string `json:"from_addr"`
		ToAddr   string `json:"to_addr"`
		TokenId  string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, TransferFromArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("transfer_from", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.TransferFrom(args.FromAddr, args.ToAddr, args.TokenId)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("transfer_from", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[AsciiArtContractState, interface{}](state, nil))
}

func TransferFrom() {
	result := coreTransferFrom()

	runtime.SetStateAndResult(result)
}

func coreGetApproved() (*[]string, errors.WeilError) {
	type GetApprovedArgs struct {
		TokenId string `json:"token_id"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, GetApprovedArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("get_approved", err)
	}

	result, err := state.GetApproved(args.TokenId)

	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("get_approved", err)
	}

	return result, nil
}

func GetApproved() {
	var resp *types.Result[[]string, errors.WeilError]

	result, err := coreGetApproved()

	if err != nil {
		resp = types.NewErrResult[[]string, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[[]string, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreIsApprovedForAll() (*bool, errors.WeilError) {
	type IsApprovedForAllArgs struct {
		Owner   string `json:"owner"`
		Spender string `json:"spender"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, IsApprovedForAllArgs]()

	if err != nil {
		return nil, errors.NewMethodArgumentDeserializationError("is_approved_for_all", err)
	}

	result := state.IsApprovedForAll(args.Owner, args.Spender)

	return &result, nil
}

func IsApprovedForAll() {
	var resp *types.Result[bool, errors.WeilError]

	result, err := coreIsApprovedForAll()

	if err != nil {
		resp = types.NewErrResult[bool, errors.WeilError](&err)
	} else {
		resp = types.NewOkResult[bool, errors.WeilError](result)
	}

	runtime.SetResult(resp)
}

func coreMint() *types.Result[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError] {
	type MintArgs struct {
		TokenId     string `json:"token_id"`
		Title       string `json:"title"`
		Name        string `json:"name"`
		Description string `json:"description"`
		Payload     string `json:"payload"`
	}

	state, args, err := runtime.StateAndArgs[AsciiArtContractState, MintArgs]()

	if err != nil {
		var newErr errors.WeilError = errors.NewMethodArgumentDeserializationError("mint", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	err = state.Mint(args.TokenId, args.Title, args.Name, args.Description, args.Payload)

	if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("mint", err)
		return types.NewErrResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](&newErr)
	}

	return types.NewOkResult[runtime.WeilValue[AsciiArtContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[AsciiArtContractState, interface{}](state, nil))
}

func Mint() {
	result := coreMint()

	runtime.SetStateAndResult(result)
}
