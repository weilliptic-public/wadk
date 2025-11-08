package fungible

import (
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/collections"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/ledger"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

type FungibleToken struct {
	Name        string                              `json:"name"`
	Symbol      string                              `json:"symbol"`
	TotalSupply uint64                              `json:"total_supply"`
	Allowances  collections.WeilMap[string, uint64] `json:"allowances"`
}

func NewFungibleToken(name string, symbol string) *FungibleToken {
	return &FungibleToken{
		Name:        name,
		Symbol:      symbol,
		TotalSupply: 0,
		Allowances:  *collections.NewWeilMap[string, uint64](*collections.NewWeilId(0)),
	}
}

func (t *FungibleToken) GetName() string {
	return t.Name
}

func (t *FungibleToken) GetSymbol() string {
	return t.Symbol
}

func (t *FungibleToken) GetTotalSupply() uint64 {
	return t.TotalSupply
}

func (t *FungibleToken) BalanceFor(addr string) (*uint64, error) {
	return ledger.BalanceFor(addr, t.Symbol)
}

func (t *FungibleToken) Transfer(toAddr string, amount uint64) error {
	sender := runtime.Sender()

	err := ledger.Transfer(t.GetSymbol(), sender, toAddr, amount)
	return err
}

func (t *FungibleToken) Approve(spender string, amount uint64) {
	sender := runtime.Sender()
	key := fmt.Sprintf(
		"%s$%s",
		sender,
		spender,
	)
	t.Allowances.Insert(&key, &amount)
}

func (t *FungibleToken) Mint(amount uint64) error {
	t.TotalSupply += amount

	sender := runtime.Sender()

	err := ledger.Mint(t.GetSymbol(), sender, amount)
	if err != nil {
		return err
	}

	return nil
}

func (t *FungibleToken) TransferFrom(fromAddr string, toAddr string, amount uint64) error {
	spender := runtime.Sender()

	key := fmt.Sprintf(
		"%s$%s",
		fromAddr,
		spender,
	)

	balance, err := t.Allowances.Get(&key)
	if err != nil {
		*balance = 0
	}

	if *balance < amount {
		return errors.NewFunctionReturnedWithError("TransferFrom", nil)
	}

	err = ledger.Transfer(t.GetSymbol(), fromAddr, toAddr, amount)
	if err != nil {
		return err
	}
	remainingBalance := *balance - amount
	t.Allowances.Insert(&key, &remainingBalance)

	return nil
}

func (t *FungibleToken) Allowance(owner string, spender string) uint64 {
	key := fmt.Sprintf(
		"%s$%s",
		owner,
		spender,
	)

	balance, err := t.Allowances.Get(&key)
	if err != nil {
		*balance = 0
	}

	return *balance
}
