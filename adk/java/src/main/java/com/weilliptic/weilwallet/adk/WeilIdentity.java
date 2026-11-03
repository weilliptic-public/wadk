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

    private final WeilClient client;
    private final String sentinelHost;

    private WeilIdentity(WeilClient client, String sentinelHost) {
        this.client = Objects.requireNonNull(client, "client");
        this.sentinelHost = sentinelHost;
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

    public WeilClient client() {
        return client;
    }

    /** The active account's canonical (sentinel-minted) address. */
    public String address() {
        return client.walletAddress();
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
        TransactionResult txn = client.execute(
            contractId,
            call.method(),
            call.argsJson(),
            call.hideArgs(),
            call.fireAndForget());
        return new WeilResult(call, contractId, txn);
    }

    /** Write a log entry to the systemic auditor applet under this identity. */
    public TransactionResult audit(String log) throws IOException, InterruptedException {
        return client.audit(log);
    }

    /** Signed auth headers, for MCP servers and other Weil-verifying services. */
    public java.util.Map<String, String> authHeaders() {
        return client.buildAuthHeaders();
    }

    @Override
    public void close() {
        client.close();
    }
}
