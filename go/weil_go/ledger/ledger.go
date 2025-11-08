package ledger

import (
	"encoding/json"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/errors"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

/*
TODO
func BalancesFor(addr string) *errors.Result[WeilTriePrefixMap[uint64]]
*/

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