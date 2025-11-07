package main

import (
	"encoding/json"
	"errors"
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/client"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/wallet"
)

type YutakaClient struct {
	client *client.WeilContractClient
}

func NewYutakaClient(contractId string, wallet *wallet.Wallet) YutakaClient {
	weilClient := client.NewWeilClient(wallet)
	weilContractClient := weilClient.ToContractClient(contractId)
	return YutakaClient{
		client: weilContractClient,
	}
}

func (cli *YutakaClient) Name() (*string, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("name", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal string

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func (cli *YutakaClient) Symbol() (*string, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("symbol", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal string

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func (cli *YutakaClient) Decimals() (*uint8, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("decimals", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal uint8

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func (cli *YutakaClient) Details() (*types.Tuple3[string, string, uint8], error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("details", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal *types.Tuple3[string, string, uint8]

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return resultVal, nil
}

func (cli *YutakaClient) TotalSupply() (*uint64, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("total_supply", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal uint64

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func (cli *YutakaClient) BalanceFor(addr string) (*uint64, error) {
	type Arg struct {
		Addr string `json:"addr"`
	}
	args := Arg{
		Addr: addr,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("balance_for", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal uint64

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func (cli *YutakaClient) Transfer(toAddr string, amount uint64) error {
	type Arg struct {
		ToAddr string `json:"to_addr"`
		Amount uint64 `json:"amount"`
	}
	args := Arg{
		ToAddr: toAddr,
		Amount: amount,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("transfer", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

func (cli *YutakaClient) Approve(spender string, amount uint64) error {
	type Arg struct {
		Spender string `json:"spender"`
		Amount  uint64 `json:"amount"`
	}
	args := Arg{
		Spender: spender,
		Amount:  amount,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("approve", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

func (cli *YutakaClient) TransferFrom(fromAddr string, toAddr string, amount uint64) error {
	type Arg struct {
		FromAddr string `json:"from_addr"`
		ToAddr   string `json:"to_addr"`
		Amount   uint64 `json:"amount"`
	}
	args := Arg{
		FromAddr: fromAddr,
		ToAddr:   toAddr,
		Amount:   amount,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("transfer_from", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

func (cli *YutakaClient) Allowance(owner string, spender string) (*uint64, error) {
	type Arg struct {
		Owner   string `json:"owner"`
		Spender string `json:"spender"`
	}
	args := Arg{
		Owner:   owner,
		Spender: spender,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("allowance", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal uint64

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func x2() (*int, error) {
	v := 3
	x := types.NewSomeOption(&v)
	if x.IsNoneResult() {
		return nil, nil
	}
	return x.TrySomeOption(), nil
}

type CounterClient struct {
	client *client.WeilContractClient
}

func NewCounterClient(contractId string, wallet *wallet.Wallet) CounterClient {
	weilClient := client.NewWeilClient(wallet)
	weilContractClient := weilClient.ToContractClient(contractId)

	return CounterClient{
		client: weilContractClient,
	}
}

func (cli *CounterClient) GetCount() (*uint32, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("get_count", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	var resultVal uint32

	err = json.Unmarshal([]byte(*resultStr), &resultVal)

	return &resultVal, nil
}

func (cli *CounterClient) Increment() error {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("increment", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

func (cli *CounterClient) SetValue(val uint32) error {
	type Arg struct {
		Val uint32 `json:"val"`
	}
	args := Arg{
		Val: val,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("set_value", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

type OptionClient struct {
	client *client.WeilContractClient
}

func NewOptionClient(contractId string, wallet *wallet.Wallet) OptionClient {
	weilClient := client.NewWeilClient(wallet)
	weilContractClient := weilClient.ToContractClient(contractId)

	return OptionClient{
		client: weilContractClient,
	}
}

func (cli *OptionClient) SetVal(val uint32) error {
	type Arg struct {
		Val uint32 `json:"val"`
	}
	args := Arg{
		Val: val,
	}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return err
	}

	txnResult, err := cli.client.Execute("set_val", string(argsBytes))
	if err != nil {
		return err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}
	return nil
}

func (cli *OptionClient) GetOption() (*uint32, error) {
	type Arg struct {
	}
	args := Arg{}

	argsBytes, err := json.Marshal(args)
	if err != nil {
		return nil, err
	}

	txnResult, err := cli.client.Execute("get_option", string(argsBytes))
	if err != nil {
		return nil, err
	}

	var result types.Result[string, string]
	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)
	if err != nil {
		return nil, err
	}
	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return nil, err
	}
	resultStr := result.TryOkResult()
	if *resultStr == "null" {
		return nil, nil
	}

	var resultVal uint32

	err = json.Unmarshal([]byte(*resultStr), &resultVal)
	if err != nil {
		return nil, err
	}

	return &resultVal, nil
}

func main() {

	wallet, _ := wallet.NewWallet("/root/.weilliptic/private_key.wc")
	contractId := "00000002b14df03428e10506946ae3e2a86217954cfc204e80f80290a72f978838e1cd4e"

	// one needs to deploy a separate counter client for this
	// countercli := NewCounterClient("00000002cd61c675d6b9d4e66c604111f0ef96196cc205b1096d930973291323ade9c150", wallet)
	// v, e := countercli.GetCount()
	// fmt.Println(*v, e)
	// e1 := countercli.Increment()
	// fmt.Println(e1)
	// v1, e2 := countercli.GetCount()
	// fmt.Println(*v1, e2)

	yut := NewYutakaClient(contractId, wallet)

	s, e := yut.Name()
	fmt.Println(*s, e)

	v2, e := yut.TotalSupply()
	fmt.Println(*v2, e)

	b, e := yut.BalanceFor("Avinash")
	if e != nil {
		fmt.Println(e)
	}
	fmt.Println(*b, e)

	e = yut.Transfer("Avinash", 200)
	fmt.Println(e)

	b1, e := yut.BalanceFor("Avinash")
	fmt.Println(*b1, e)

	// one needs to deploy a separate option contract for this. It is avialable in examples git repo, in the branch : "divyansh/option_example"
	optionContractId := "00000002884553c4cea774877211bfc07403123e49349395b9ca601a4abab37cd0ee03da"
	optionCli := NewOptionClient(optionContractId, wallet)
	v3, e := optionCli.GetOption()
	fmt.Println("options start: ", v3, e)
	optionCli.SetVal(2)
	v4, e := optionCli.GetOption()
	fmt.Println(v4, e)
}
