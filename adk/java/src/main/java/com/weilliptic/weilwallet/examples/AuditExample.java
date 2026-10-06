package com.weilliptic.weilwallet.examples;

import com.weilliptic.weilwallet.SelectedAccount;
import com.weilliptic.weilwallet.Wallet;
import com.weilliptic.weilwallet.WeilClient;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Example: initialize wallet from wallet.wc and submit an audit log to WeilChain.
 *
 * Run with: mvn exec:java@audit-example
 * Place wallet.wc in the project root or working directory.
 */
public class AuditExample {

    public static void main(String[] args) throws IOException, InterruptedException {
        Path walletPath = findWalletWcPath();
        Wallet wallet = Wallet.fromWalletFile(walletPath);
        System.out.println("Wallet loaded from " + walletPath);
        System.out.println("  derived accounts:  " + wallet.derivedAccountCount());
        System.out.println("  external accounts: " + wallet.externalAccountCount());
        System.out.println("  active account:    " + wallet.currentAccountIndex());

        String sentinel = System.getenv("SENTINEL_HOST");
        try (WeilClient client = new WeilClient(wallet, sentinel)) {

            // Execute with the default account.
            System.out.println("Executing audit log");
            TransactionResult result = client.audit("Hello from Java!");
            System.out.println("Result:");
            System.out.println("  status:        " + result.getStatus());
            System.out.println("  txn_result:    " + result.getTxnResult());

            // Switch to derived account 1 if available.
            if (wallet.derivedAccountCount() > 1) {
                client.setAccount(SelectedAccount.Derived(1));
                System.out.println("Switched to derived account 1");
                client.audit("Hello from Java! (account 1)");
                System.out.println("Audit from account 1 submitted.");
            }
        }
    }

    private static Path findWalletWcPath() throws IOException {
        Path cwd = Paths.get(System.getProperty("user.dir"));
        Path[] candidates = {
            cwd.resolve("wallet.wc"),
            cwd.resolve("src/main/java/com/weilliptic/weilwallet/examples/wallet.wc"),
            cwd.getParent() != null ? cwd.getParent().resolve("wallet.wc") : null
        };
        for (Path p : candidates) {
            if (p != null && Files.isRegularFile(p)) {
                return p;
            }
        }
        throw new IOException(
            "wallet.wc not found. Place it in the project root or examples/ directory.");
    }
}
