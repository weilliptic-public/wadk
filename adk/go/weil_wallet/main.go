package main

import (
	"fmt"

	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/client"
	"github.com/weilliptic-inc/contract-sdk/go/weil_wallet/wallet"
)

func main() {
	wallet, _ := wallet.NewWallet("private_key.wc")
	client := client.NewWeilClient(wallet)

	err := client.Audit("This is my super log")

	if err != nil {
		fmt.Println(err)
	}
}
