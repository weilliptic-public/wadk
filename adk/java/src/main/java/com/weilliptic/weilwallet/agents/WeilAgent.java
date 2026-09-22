package com.weilliptic.weilwallet.agents;

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
 * Use {@link #setWalletPath(String)} to set or change the wallet (private key file).
 */
public class WeilAgent<T> {

    private final T agent;
    private Wallet wallet;
    private WeilClient client;
    private String sentinelHost;

    /**
     * Create a WeilAgent with no wallet pre-configured.
     * Call {@link #setWalletPath(String)} before invoking {@link #audit(String)}.
     *
     * @param agent the underlying agent implementation to wrap.
     */
    public WeilAgent(T agent) {
        this(agent, (String) null, (Wallet) null, null);
    }

    /**
     * Create a WeilAgent that loads its wallet from a private key file.
     *
     * @param agent          the underlying agent implementation to wrap.
     * @param privateKeyPath path to the {@code wallet.wc} file; {@code null} defers setup.
     */
    public WeilAgent(T agent, String privateKeyPath) {
        this(agent, privateKeyPath, null, null);
    }

    /**
     * Create a WeilAgent with an already-constructed wallet.
     *
     * @param agent  the underlying agent implementation to wrap.
     * @param wallet the pre-loaded wallet to use for signing and auditing.
     */
    public WeilAgent(T agent, Wallet wallet) {
        this(agent, null, wallet, null);
    }

    /**
     * Create a WeilAgent that loads its wallet from a private key file
     * and targets a specific Sentinel host.
     *
     * @param agent          the underlying agent implementation to wrap.
     * @param privateKeyPath path to the {@code wallet.wc} file; {@code null} defers setup.
     * @param sentinelHost   Sentinel host to use; {@code null} falls back to {@code SENTINEL_HOST} env var.
     */
    public WeilAgent(T agent, String privateKeyPath, String sentinelHost) {
        this(agent, privateKeyPath, null, sentinelHost);
    }

    /**
     * Create a WeilAgent with optional wallet, private key file, and Sentinel host.
     *
     * <p>A pre-built {@code wallet} takes precedence over {@code privateKeyPath}.
     * When both are {@code null}, no wallet is configured and
     * {@link #setWalletPath(String)} must be called before {@link #audit(String)}.</p>
     *
     * @param agent          the underlying agent implementation to wrap.
     * @param privateKeyPath path to the {@code wallet.wc} file; may be {@code null}.
     * @param wallet         a pre-loaded wallet; may be {@code null}.
     * @param sentinelHost   Sentinel host to use; {@code null} falls back to {@code SENTINEL_HOST} env var.
     */
    public WeilAgent(T agent, String privateKeyPath, Wallet wallet, String sentinelHost) {
        this.agent = agent;
        this.sentinelHost = sentinelHost != null ? sentinelHost : System.getenv("SENTINEL_HOST");
        if (wallet != null) {
            this.wallet = wallet;
        } else if (privateKeyPath != null && !privateKeyPath.isEmpty()) {
            setWalletPath(privateKeyPath);
        }
    }

    /**
     * Set or change the wallet identity from a private key file.
     * The next audit() will use the new wallet. Any existing client is discarded.
     */
    public void setWalletPath(String path) {
        setWalletPath(Paths.get(path));
    }

    /**
     * Set or change the wallet identity from a {@link Path}.
     * The existing {@link WeilClient} is discarded and will be recreated on the next
     * call to {@link #audit(String)}.
     *
     * @param path path to the {@code wallet.wc} file.
     * @throws IllegalArgumentException if the file does not exist.
     * @throws RuntimeException         if the wallet file cannot be parsed.
     */
    public void setWalletPath(Path path) {
        if (!Files.isRegularFile(path)) {
            throw new IllegalArgumentException("Account file not found: " + path);
        }
        try {
            this.wallet = Wallet.fromWalletFile(path);
            this.client = null;
        } catch (IOException e) {
            throw new RuntimeException("Failed to load account from " + path, e);
        }
    }

    /**
     * Replace the wallet with an already-constructed instance.
     * The existing {@link WeilClient} is discarded and will be recreated lazily.
     *
     * @param wallet the new wallet to use for signing and auditing.
     */
    public void setWallet(Wallet wallet) {
        this.wallet = wallet;
        this.client = null;
    }

    /**
     * Return the configured wallet, throwing if none has been set.
     *
     * @throws IllegalStateException if no wallet has been configured.
     */
    private Wallet ensureWallet() {
        if (wallet == null) {
            throw new IllegalStateException(
                "No wallet set. Call setWalletPath(path) or create the agent with privateKeyPath or wallet.");
        }
        return wallet;
    }

    /**
     * Return the {@link WeilClient}, creating it lazily from the current wallet.
     */
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

    /**
     * Return the current wallet, or {@code null} if none has been set.
     */
    public Wallet getWallet() {
        return wallet;
    }

    /**
     * Find {@code private_key.wc} in the default locations: cwd, then parent, then {@code examples/}.
     *
     * @return the first existing {@code private_key.wc} path.
     * @throws IllegalStateException if the file is not found in any default location.
     */
    public static Path findDefaultPrivateKeyPath() {
        Path cwd = Paths.get("").toAbsolutePath();
        Path[] candidates = {
            cwd.resolve("private_key.wc"),
            cwd.getParent() != null ? cwd.getParent().resolve("private_key.wc") : null,
            cwd.resolve("examples").resolve("private_key.wc")
        };
        for (Path p : candidates) {
            if (p != null && Files.isRegularFile(p)) {
                return p;
            }
        }
        throw new IllegalStateException("private_key.wc not found. Place it in cwd, project root, or examples/.");
    }
}
