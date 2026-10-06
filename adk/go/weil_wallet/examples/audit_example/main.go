// Run from the weil_wallet module root:
//
//	go run ./examples/audit_example
//
// Place wallet.wc in go/weil_wallet/wallet.wc
package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/wallet"
)

func main() {
	walletPath := filepath.Join("./", "wallet.wc")

	w, err := wallet.FromWalletFile(walletPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "wallet:", err)
		os.Exit(1)
	}
	fmt.Printf("Wallet loaded: %d derived account(s), %d external account(s)\n",
		w.DerivedAccountCount(), w.ExternalAccountCount())

	cli := client.NewWeilClient(w)

	fmt.Println("Executing audit log with default account")
	if err := cli.Audit("Hello from Go!"); err != nil {
		fmt.Fprintln(os.Stderr, "audit:", err)
		os.Exit(1)
	}
	fmt.Println("Audit submitted successfully.")

	// Switch to derived account 1 (if present) and audit again.
	if w.DerivedAccountCount() > 1 {
		if err := cli.SetAccount(wallet.Derived(1)); err != nil {
			fmt.Fprintln(os.Stderr, "set account:", err)
			os.Exit(1)
		}
		fmt.Println("Switched to derived account 1")
		if err := cli.Audit("Hello from Go! (account 1)"); err != nil {
			fmt.Fprintln(os.Stderr, "audit:", err)
			os.Exit(1)
		}
		fmt.Println("Audit submitted from account 1.")
	}
}
