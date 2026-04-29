package com.weilliptic.weilwallet.examples;

import com.weilliptic.weilwallet.PrivateKey;
import com.weilliptic.weilwallet.Wallet;
import com.weilliptic.weilwallet.WeilClient;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Example: initialize wallet from wallet.wc (multi-account export JSON) and submit an audit log to WeilChain.
 *
 * Run with: mvn exec:java -Dexec.mainClass="com.weilliptic.weilwallet.examples.AuditExample"
 * Or place private_key.wc in examples/, project root, or cwd.
 */
public class AuditExample {

    public static void main(String[] args) throws IOException, InterruptedException {
        Path keyPath = findAccountExportPath();
        Wallet wallet = Wallet.fromWalletFile(keyPath);
        System.out.println("Wallet initialized from wallet.wc");

        String sentinel = System.getenv("SENTINEL_HOST");
        try (WeilClient client = new WeilClient(wallet, sentinel)) {
            System.out.println("Executing audit log");
            TransactionResult result = client.audit("Hello from java!");
            System.out.println("Result:");
            System.out.println("  status:        " + result.getStatus());
            System.out.println("  block_height:  " + result.getBlockHeight());
            System.out.println("  batch_id:      " + result.getBatchId());
            System.out.println("  tx_idx:        " + result.getTxIdx());
            System.out.println("  txn_result:    " + result.getTxnResult());
            System.out.println("  creation_time: " + result.getCreationTime());
        }
    }

    private static Path findAccountExportPath() throws IOException {
        Path scriptDir = Paths.get(System.getProperty("user.dir"));
        Path[] candidates = {
            scriptDir.resolve("wallet.wc"),
            scriptDir.resolve("examples").resolve("wallet.wc"),
            scriptDir.getParent() != null ? scriptDir.getParent().resolve("wallet.wc") : null
        };
        for (Path p : candidates) {
            if (p != null && Files.isRegularFile(p)) {
                return p;
            }
        }
        throw new IOException("wallet.wc not found. Place it in examples/, project root, or cwd.");
    }
}
