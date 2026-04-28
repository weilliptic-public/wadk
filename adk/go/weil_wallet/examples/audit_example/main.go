// Run from the weil_wallet module root:
//   go run ./examples/audit_example
//
// Place a multi-account wallet export JSON in go/weil_wallet/wallet.wc

package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

func main() {
	// Assume wallet.wc is in the go/weil_wallet folder
	keyPath := filepath.Join("./", "wallet.wc")

	w, err := wallet.NewWalletFromWalletFile(keyPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "wallet:", err)
		os.Exit(1)
	}
	fmt.Println("Wallet initialized from wallet.wc")

	cli := client.NewWeilClient(w)
	fmt.Println("Executing audit log")
	if err := cli.Audit("Hello from Go!"); err != nil {
		fmt.Fprintln(os.Stderr, "audit:", err)
		os.Exit(1)
	}
	fmt.Println("Audit submitted successfully.")
}
