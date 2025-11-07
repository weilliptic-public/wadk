// Package fungible provides a fungible token implementation for Weil contracts.
// It implements a standard ERC-20-like token with transfer, approval, and minting capabilities.
package fungible

import (
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/collections"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/ledger"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

// FungibleToken represents a fungible token contract with name, symbol, total supply, and allowances.
type FungibleToken struct {
	Name        string                              `json:"name"`
	Symbol      string                              `json:"symbol"`
	TotalSupply uint64                              `json:"total_supply"`
	Allowances  collections.WeilMap[string, uint64] `json:"allowances"`
}

// NewFungibleToken creates a new fungible token with the given name and symbol.
// The total supply is initialized to 0.
func NewFungibleToken(name string, symbol string) *FungibleToken {
	return &FungibleToken{
		Name:        name,
		Symbol:      symbol,
		TotalSupply: 0,
		Allowances:  *collections.NewWeilMap[string, uint64](*collections.NewWeilId(0)),
	}
}

// GetName returns the name of the token.
func (t *FungibleToken) GetName() string {
	return t.Name
}

// GetSymbol returns the symbol of the token.
func (t *FungibleToken) GetSymbol() string {
	return t.Symbol
}

// GetTotalSupply returns the total supply of the token.
func (t *FungibleToken) GetTotalSupply() uint64 {
	return t.TotalSupply
}

// BalanceFor retrieves the balance of the token for the given address.
func (t *FungibleToken) BalanceFor(addr string) (*uint64, error) {
	return ledger.BalanceFor(addr, t.Symbol)
}

// Transfer transfers tokens from the sender's address to the specified address.
// Returns an error if the transfer fails (e.g., insufficient balance).
func (t *FungibleToken) Transfer(toAddr string, amount uint64) error {
	sender := runtime.Sender()

	err := ledger.Transfer(t.GetSymbol(), sender, toAddr, amount)
	return err
}

// Approve approves the spender to transfer tokens from the sender's address.
// The approval amount is stored in the allowances map.
func (t *FungibleToken) Approve(spender string, amount uint64) {
	sender := runtime.Sender()
	key := fmt.Sprintf(
		"%s$%s",
		sender,
		spender,
	)
	t.Allowances.Insert(&key, &amount)
}

// Mint creates new tokens and assigns them to the sender's address.
// The total supply is incremented by the minted amount.
// Returns an error if the minting operation fails.
func (t *FungibleToken) Mint(amount uint64) error {
	t.TotalSupply += amount

	sender := runtime.Sender()

	err := ledger.Mint(t.GetSymbol(), sender, amount)
	if err != nil {
		return err
	}

	return nil
}

// TransferFrom transfers tokens from one address to another on behalf of the sender.
// The sender must have been approved to transfer at least the specified amount.
// Returns an error if the transfer fails (e.g., insufficient balance or allowance).
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

// Allowance returns the amount of tokens that the spender is approved to transfer
// from the owner's address. Returns 0 if no allowance has been set.
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
