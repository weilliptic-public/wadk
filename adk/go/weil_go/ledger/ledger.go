// Package ledger provides functions for interacting with the ledger contract.
// The ledger contract manages token balances and transfers.
package ledger

import (
	"encoding/json"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

/*
TODO
func BalancesFor(addr string) *errors.Result[WeilTriePrefixMap[uint64]]
*/

// BalanceFor retrieves the balance of a specific token symbol for the given address.
// Returns the balance as a uint64, or an error if the query fails.
func BalanceFor(addr string, symbol string) (*uint64, error) {
	type BalanceForArgs struct {
		Addr   string `json:"addr"`
		Symbol string `json:"symbol"`
	}

	args, _ := json.Marshal(BalanceForArgs{
		Addr:   addr,
		Symbol: symbol,
	})

	ledgerId := runtime.LedgerContractId()

	result, err := runtime.CallContract[uint64](ledgerId, "balance_for", string(args))
	if err != nil {
		return nil, errors.NewFunctionReturnedWithError("balance_for", err)
	}

	return result, nil
}

// Transfer transfers tokens of the given symbol from one address to another.
// Returns an error if the transfer fails (e.g., insufficient balance).
func Transfer(symbol string, fromAddr string, toAddr string, amount uint64) error {
	type TransferArgs struct {
		Symbol   string `json:"symbol"`
		FromAddr string `json:"from_addr"`
		ToAddr   string `json:"to_addr"`
		Amount   uint64 `json:"amount"`
	}

	args, _ := json.Marshal(TransferArgs{
		Symbol:   symbol,
		FromAddr: fromAddr,
		ToAddr:   toAddr,
		Amount:   amount,
	})

	ledgerId := runtime.LedgerContractId()

	_, err := runtime.CallContract[uint64](ledgerId, "transfer", string(args))
	if err != nil {
		return errors.NewFunctionReturnedWithError("transfer", err)
	}

	return nil
}

// Mint creates new tokens of the given symbol and assigns them to the specified address.
// Returns an error if the minting operation fails.
func Mint(symbol string, toAddr string, amount uint64) error {
	type MintArgs struct {
		Symbol string `json:"symbol"`
		ToAddr string `json:"to_addr"`
		Amount uint64 `json:"amount"`
	}

	args, _ := json.Marshal(MintArgs{
		Symbol: symbol,
		ToAddr: toAddr,
		Amount: amount,
	})

	ledgerId := runtime.LedgerContractId()

	_, err := runtime.CallContract[uint64](ledgerId, "mint", string(args))
	if err != nil {
		return errors.NewFunctionReturnedWithError("mint", err)
	}

	return nil
}
