package com.weilliptic.weilwallet.agents;

import com.weilliptic.weilwallet.PrivateKey;
import com.weilliptic.weilwallet.Wallet;
import com.weilliptic.weilwallet.WeilClient;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Wraps an agent with a Weil identity (wallet) and audit capability.
 * Call {@link #audit(String)} to record a log entry on-chain.
 * Use {@link #setWalletPath(String)} to set or change the wallet (wallet.wc file).
 */
public class WeilAgent<T> {

    private final T agent;
    private Wallet wallet;
    private WeilClient client;
    private String sentinelHost;

    public WeilAgent(T agent) {
        this(agent, (String) null, (Wallet) null, null);
    }

    public WeilAgent(T agent, String accountExportPath) {
        this(agent, accountExportPath, null, null);
    }

    public WeilAgent(T agent, Wallet wallet) {
        this(agent, null, wallet, null);
    }

    public WeilAgent(T agent, String accountExportPath, String sentinelHost) {
        this(agent, accountExportPath, null, sentinelHost);
    }

    public WeilAgent(T agent, String accountExportPath, Wallet wallet, String sentinelHost) {
        this.agent = agent;
        this.sentinelHost = sentinelHost != null ? sentinelHost : System.getenv("SENTINEL_HOST");
        if (wallet != null) {
            this.wallet = wallet;
        } else if (accountExportPath != null && !accountExportPath.isEmpty()) {
            setWalletPath(accountExportPath);
        }
    }

    /**
     * Set or change the wallet identity from a private key file.
     * The next audit() will use the new wallet. Any existing client is discarded.
     */
    public void setWalletPath(String path) {
        setWalletPath(Paths.get(path));
    }

    public void setWalletPath(Path path) {
        if (!Files.isRegularFile(path)) {
            throw new IllegalArgumentException("Wallet file not found: " + path);
        }
        try {
            this.wallet = Wallet.fromWalletFile(path);
            this.client = null;
        } catch (IOException e) {
            throw new RuntimeException("Failed to load wallet from " + path, e);
        }
    }

    public void setWallet(Wallet wallet) {
        this.wallet = wallet;
        this.client = null;
    }

    private Wallet ensureWallet() {
        if (wallet == null) {
            throw new IllegalStateException(
                "No wallet set. Call setWalletPath(path) or create the agent with accountExportPath or wallet.");
        }
        return wallet;
    }

    private WeilClient getClient() {
        if (client == null) {
            client = new WeilClient(ensureWallet(), sentinelHost);
        }
        return client;
    }

    /**
     * Record an audit log entry on-chain for this agent's identity.
     */
    public TransactionResult audit(String log) throws IOException, InterruptedException {
        return getClient().audit(log);
    }

    /**
     * Return the wrapped agent for method calls.
     */
    public T getAgent() {
        return agent;
    }

    public Wallet getWallet() {
        return wallet;
    }

    /**
     * Default locations to look for wallet.wc (cwd, then parent, then examples/).
     */
    public static Path findDefaultAccountExportPath() {
        Path cwd = Paths.get("").toAbsolutePath();
        Path[] candidates = {
            cwd.resolve("wallet.wc"),
            cwd.getParent() != null ? cwd.getParent().resolve("wallet.wc") : null,
            cwd.resolve("examples").resolve("wallet.wc")
        };
        for (Path p : candidates) {
            if (p != null && Files.isRegularFile(p)) {
                return p;
            }
        }
        throw new IllegalStateException("wallet.wc not found. Place it in cwd, project root, or examples/.");
    }
}
