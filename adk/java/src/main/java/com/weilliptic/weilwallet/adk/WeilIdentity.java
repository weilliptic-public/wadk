package com.weilliptic.weilwallet.adk;

import com.weilliptic.weilwallet.ContractId;
import com.weilliptic.weilwallet.Wallet;
import com.weilliptic.weilwallet.WeilClient;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;
import java.util.Objects;

/**
 * An agent's on-chain identity: a wallet, and a client that signs and submits
 * applet calls with it.
 *
 * <p>Mirrors {@code weiladk.Identity} in the Go SDK. Everything an agent does
 * on chain through this identity is signed by its wallet, so the activity is
 * attributable to that agent and nobody else can act in its name.</p>
 */
public final class WeilIdentity implements AutoCloseable {

    /** Loads the wallet's client: from a key, a file, or anything else. */
    @FunctionalInterface
    public interface Loader {
        WeilClient load() throws IOException, InterruptedException;
    }

    private final Loader loader;
    private final String sentinelHost;
    private volatile WeilClient client;

    private WeilIdentity(WeilClient client, String sentinelHost) {
        this.client = Objects.requireNonNull(client, "client");
        this.loader = null;
        this.sentinelHost = sentinelHost;
    }

    private WeilIdentity(Loader loader, String sentinelHost) {
        this.loader = Objects.requireNonNull(loader, "loader");
        this.sentinelHost = sentinelHost;
    }

    /**
     * An identity whose wallet is loaded on first use, not here: the first
     * call that needs it, such as a signature or an address, runs
     * {@code loader}. A server can then start and answer before the wallet is
     * reachable, or with a placeholder key, as a container health check does.
     * A load that fails throws from that call, and the next call tries again.
     */
    public static WeilIdentity deferred(Loader loader, String sentinelHost) {
        return new WeilIdentity(loader, sentinelHost);
    }

    /** Wrap an existing client. */
    public static WeilIdentity of(WeilClient client) {
        return new WeilIdentity(client, null);
    }

    /** Load an identity from a wallet file. */
    public static WeilIdentity fromWalletFile(String path) throws IOException {
        return new WeilIdentity(WeilClient.fromWalletFile(path), null);
    }

    /**
     * Resolve an Agent Registry API key to the agent's identity.
     *
     * <p>The wallet is masked: no private key is held locally and Sentinel
     * signs on the agent's behalf.</p>
     */
    public static WeilIdentity fromApiKey(String apiKey) throws IOException, InterruptedException {
        return new WeilIdentity(WeilClient.fromApiKey(apiKey), null);
    }

    public static WeilIdentity fromApiKey(String apiKey, String sentinelHost)
        throws IOException, InterruptedException {
        return new WeilIdentity(WeilClient.fromApiKey(apiKey, null, sentinelHost), sentinelHost);
    }

    /**
     * The wallet's client, loading it first if this identity is
     * {@link #deferred}.
     *
     * @throws IllegalStateException when the wallet cannot be loaded
     */
    public WeilClient client() {
        WeilClient loaded = client;
        if (loaded != null) {
            return loaded;
        }
        synchronized (this) {
            if (client == null) {
                try {
                    client = loader.load();
                } catch (IOException e) {
                    throw new IllegalStateException(
                        "cannot load the agent's wallet: " + e.getMessage(), e);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IllegalStateException("interrupted loading the agent's wallet", e);
                }
            }
            return client;
        }
    }

    /** Whether the wallet is loaded: always for an identity that is not {@link #deferred}. */
    public boolean isLoaded() {
        return client != null;
    }

    /** The active account's canonical (sentinel-minted) address. */
    public String address() {
        return client().walletAddress();
    }

    String sentinelHost() {
        return sentinelHost;
    }

    /**
     * Sign and submit {@code call} as this identity.
     *
     * <p>Unless the call is fire-and-forget, this waits for the applet method
     * to run.</p>
     */
    public WeilResult invoke(Call call) throws IOException, InterruptedException {
        Objects.requireNonNull(call, "call");
        ContractId contractId = call.applet().resolve(sentinelHost, address());
        TransactionResult txn = client().execute(
            contractId,
            call.method(),
            call.argsJson(),
            call.hideArgs(),
            call.fireAndForget());
        return new WeilResult(call, contractId, txn);
    }

    /** Write a log entry to the systemic auditor applet under this identity. */
    public TransactionResult audit(String log) throws IOException, InterruptedException {
        return client().audit(log);
    }

    /** Signed auth headers, for MCP servers and other Weil-verifying services. */
    public java.util.Map<String, String> authHeaders() {
        return client().buildAuthHeaders();
    }

    /** Closes the wallet's client; a deferred wallet never loaded has nothing to close. */
    @Override
    public void close() {
        WeilClient loaded = client;
        if (loaded != null) {
            loaded.close();
        }
    }
}
