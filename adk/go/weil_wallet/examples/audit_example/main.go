// Run from the weil_wallet module root:
//   go run ./examples/audit_example
//
// Place private_key.wc in go/weil_wallet/private_key.wc

package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

func main() {
	// Assume private_key.wc is in the go/weil_wallet folder
	keyPath := filepath.Join("./", "private_key.wc")

	w, err := wallet.NewWallet(keyPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "wallet:", err)
		os.Exit(1)
	}
	fmt.Println("Wallet initialized from private_key.wc")

	cli := client.NewWeilClient(w)
	fmt.Println("Executing audit log")
	if err := cli.Audit("Hello from Go!"); err != nil {
		fmt.Fprintln(os.Stderr, "audit:", err)
		os.Exit(1)
	}
	fmt.Println("Audit submitted successfully.")
}
