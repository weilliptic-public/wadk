package main

import (
	"fmt"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

func main() {
	wallet, err := wallet.FromWalletFile("/root/.weilliptic/wallet.wc")

	if err != nil {
		return
	}

	client := client.NewWeilClient(wallet)

	res, err := client.Execute("aaaaaaskpkvu7oswv7eccsj5h3euom5euj3sjgjwy6grgothpctmcxftne", "audit", "{\"log\": \"Bhavya Bro is man!\"}", false, false)

	if err != nil {
		return
	}

	fmt.Println(res)
}
