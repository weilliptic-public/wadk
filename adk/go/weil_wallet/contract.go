package main

import (
	"encoding/json"
	"errors"

	"github.com/weilliptic-inc/contract-sdk/go/weil_go/types"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/client"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/wallet"
)

type AuditMeClient struct {
	client *client.WeilContractClient
}

func NewAuditMeClient(contractId string, wallet *wallet.Wallet) *AuditMeClient {
	weilClient := client.NewWeilClient(wallet)
	weilContractClient := weilClient.ToContractClient(contractId)

	return &AuditMeClient{
		client: weilContractClient,
	}
}

func (cli *AuditMeClient) Audit(log string) error {
	type Arg struct {
		Log string `json:"log"`
	}
	args := Arg{
		Log: log,
	}

	argsBytes, err := json.Marshal(args)

	if err != nil {
		return err
	}
	txnResult, err := cli.client.Execute("audit", string(argsBytes), true, false)

	if err != nil {
		return err
	}
	var result types.Result[string, string]

	err = json.Unmarshal([]byte(txnResult.TxnResult), &result)

	if err != nil {
		err = errors.New(err.Error() + txnResult.TxnResult)
		return err
	}

	if result.IsErrResult() {
		err = errors.New(*result.TryErrResult())
		return err
	}

	return nil
}
