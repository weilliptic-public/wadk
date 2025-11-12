package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/weil_contracts/fungible"
)

const total_supply = 100_000_000_000

type YutakaContractState struct {
	Inner fungible.FungibleToken
}

func NewYutakaContractState() (*YutakaContractState, error) {
	euler := YutakaContractState{
		Inner: *fungible.NewFungibleToken("Yutaka", "YTK"),
	}
	err := euler.Inner.Mint(total_supply)
	if err != nil {
		return nil, err
	}
	return &euler, nil
}

// query
func (obj *YutakaContractState) Name() string {
	result := obj.Inner.GetName()
	return result
}

// query
func (obj *YutakaContractState) Symbol() string {
	result := obj.Inner.GetSymbol()
	return result
}

// query
func (obj *YutakaContractState) Decimals() uint8 {
	return 6
}

// query
func (obj *YutakaContractState) Details() *types.Tuple3[string, string, uint8] {
	result := &types.Tuple3[string, string, uint8]{
		F0: obj.Inner.GetName(),
		F1: obj.Inner.GetSymbol(),
		F2: obj.Decimals(),
	}
	return result
}

// query
func (obj *YutakaContractState) TotalSupply() uint64 {
	result := obj.Inner.GetTotalSupply()
	return result
}

// query
func (obj *YutakaContractState) BalanceFor(addr string) (*uint64, error) {
	result, err := obj.Inner.BalanceFor(addr)

	if err != nil {
		return nil, err
	}

	return result, nil
}

// mutate
func (obj *YutakaContractState) Transfer(toAddr string, amount uint64) error {
	err := obj.Inner.Transfer(toAddr, amount)
	return err
}

// mutate
func (obj *YutakaContractState) Approve(spender string, amount uint64) {
	obj.Inner.Approve(spender, amount)
}

// mutate
func (obj *YutakaContractState) TransferFrom(fromAddr string, toAddr string, amount uint64) error {
	err := obj.Inner.TransferFrom(fromAddr, toAddr, amount)
	return err
}

// query
func (obj *YutakaContractState) Allowance(owner string, spender string) uint64 {
	result := obj.Inner.Allowance(owner, spender)
	return result
}
