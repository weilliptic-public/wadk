// Run from the weil_wallet module root:
//   go run ./examples/audit_example
//
// Place an exported account JSON in go/weil_wallet/account.wc

package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

func main() {
	// Assume account.wc (sentinel export JSON) is in the go/weil_wallet folder
	keyPath := filepath.Join("./", "account.wc")

	w, err := wallet.NewWalletFromAccountExportFile(keyPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "wallet:", err)
		os.Exit(1)
	}
	fmt.Println("Wallet initialized from account.wc")

	cli := client.NewWeilClient(w)
	fmt.Println("Executing audit log")
	if err := cli.Audit("Hello from Go!"); err != nil {
		fmt.Fprintln(os.Stderr, "audit:", err)
		os.Exit(1)
	}
	fmt.Println("Audit submitted successfully.")
}
