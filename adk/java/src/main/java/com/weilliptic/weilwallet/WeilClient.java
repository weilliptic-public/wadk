package com.weilliptic.weilwallet;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.weilliptic.weilwallet.api.*;
import com.weilliptic.weilwallet.transaction.TransactionHeader;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.Semaphore;

/**
 * High-level client for WeilChain applet methods.
 *
 * <p>Holds an HTTP client, a signer {@link Wallet}, and a concurrency-limiting
 * {@link Semaphore}. Thread-safe: wallet access and audit contract ID resolution
 * are both synchronized.</p>
 *
 * <p>Typical usage:</p>
 * <pre>{@code
 * WeilClient client = WeilClient.fromWalletFile("wallet.wc");
 * TransactionResult result = client.execute(contractId, "myMethod", "{\"key\":\"value\"}", false);
 * }</pre>
 *
 * <p>Implements {@link AutoCloseable} for use in try-with-resources blocks.</p>
 */
public class WeilClient implements AutoCloseable {

    private static final ObjectMapper JSON = new ObjectMapper();
    private static final String AUDIT_APPLET_SVC_NAME = "auditor";

    private final Wallet wallet;
    private final Object walletLock = new Object();
    private final String sentinelHost;
    private final HttpClient httpClient;
    private final Semaphore semaphore;
    private volatile ContractId auditContractId;

    /**
     * Create a WeilClient with the given wallet, using the default sentinel host
     * and default concurrency.
     *
     * @param wallet the signing wallet.
     */
    public WeilClient(Wallet wallet) {
        this(wallet, null, null);
    }

    /**
     * Create a WeilClient with the given wallet and a custom sentinel host.
     *
     * @param wallet       the signing wallet.
     * @param sentinelHost base URL of the Sentinel node (e.g. {@code "https://sentinel.unweil.me"}).
     */
    public WeilClient(Wallet wallet, String sentinelHost) {
        this(wallet, null, sentinelHost);
    }

    /**
     * Create a WeilClient with full control over concurrency and sentinel host.
     *
     * @param wallet       the signing wallet.
     * @param concurrency  max concurrent in-flight requests; {@code null} uses the default.
     * @param sentinelHost base URL of the Sentinel node; {@code null} uses the default.
     */
    public WeilClient(Wallet wallet, Integer concurrency, String sentinelHost) {
        this.wallet = wallet;
        this.sentinelHost = sentinelHost != null && !sentinelHost.isEmpty() ? sentinelHost : Constants.SENTINEL_HOST;
        this.httpClient = HttpClient.newBuilder().build();
        int conc = concurrency != null ? concurrency : Constants.DEFAULT_CONCURRENCY;
        this.semaphore = new Semaphore(conc);
        this.auditContractId = null;
    }

    /**
     * Construct a WeilClient from a single-account export file ({@code account.wc}).
     *
     * @param path path to the account export JSON file.
     * @throws IOException if the file cannot be read or parsed.
     */
    public static WeilClient fromAccountExportFile(String path) throws IOException {
        Wallet w = Wallet.fromAccountExportFile(java.nio.file.Paths.get(path));
        return new WeilClient(w);
    }

    /**
     * Construct a WeilClient from a multi-account wallet file ({@code wallet.wc}).
     * Derived accounts are re-derived from the stored xprv; external accounts are
     * read directly from the file.
     *
     * @param path path to the wallet file.
     * @throws IOException if the file cannot be read or parsed.
     */
    public static WeilClient fromWalletFile(String path) throws IOException {
        Wallet w = Wallet.fromWalletFile(java.nio.file.Paths.get(path));
        return new WeilClient(w);
    }

    /**
     * Append an additional account from a sentinel account export file.
     * The active account does not change. Thread-safe.
     *
     * @param path path to the account export JSON file.
     * @throws IOException if the file cannot be read or parsed.
     */
    public void addAccountFromExportFile(String path) throws IOException {
        synchronized (walletLock) {
            wallet.addAccountFromExportFile(java.nio.file.Paths.get(path));
        }
    }

    /**
     * Switch the active signing account. Thread-safe.
     *
     * @param selected identifies the account to activate (use {@link SelectedAccount#derived(int)}
     *                 or {@link SelectedAccount#external(int)}).
     * @throws IllegalArgumentException if the index is out of bounds.
     */
    public void setAccount(SelectedAccount selected) {
        synchronized (walletLock) {
            wallet.setIndex(selected);
        }
    }

    /**
     * Resolve and cache the audit applet contract address from the Sentinel API.
     */
    private synchronized ContractId getAuditContractId() throws IOException, InterruptedException {
        if (auditContractId != null) {
            return auditContractId;
        }
        String url = sentinelHost.replaceAll("/$", "") + "/get_applet_address";
        String body = JSON.writeValueAsString(Map.of("svc_name", AUDIT_APPLET_SVC_NAME));
        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(url))
            .timeout(Duration.ofSeconds(30))
            .header("Content-Type", "application/json")
            .POST(HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8))
            .build();
        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw new RuntimeException("get_applet_address failed: HTTP " + response.statusCode() + " " + response.body());
        }
        Map<String, Object> data = JSON.readValue(response.body(), new TypeReference<Map<String, Object>>() {});
        if (data.containsKey("Ok")) {
            Object ok = data.get("Ok");
            String address = ok != null ? ok.toString() : null;
            if (address != null && !address.isEmpty()) {
                auditContractId = ContractId.of(address);
                return auditContractId;
            }
        }
        throw new RuntimeException("get_applet_address failed: " + data.getOrDefault("Err", data));
    }

    /**
     * Execute the audit contract method and return the transaction result.
     */
    public TransactionResult audit(String log) throws IOException, InterruptedException {
        ContractId contractId = getAuditContractId();
        String methodName = "audit";
        String methodArgs = JSON.writeValueAsString(Map.of("log", log != null ? log : ""));
        return execute(contractId, methodName, methodArgs, false);
    }

    /**
     * Execute a contract method and return the transaction result.
     *
     * <p>Builds and signs a {@code SmartContractExecutor} transaction, then submits
     * it to the platform API. Concurrency is bounded by the internal semaphore.</p>
     *
     * @param contractId     the target applet's on-chain address.
     * @param methodName     the exported method to invoke.
     * @param methodArgs     JSON-encoded argument payload.
     * @param shouldHideArgs when {@code true} the arguments are encrypted before submission.
     * @return the transaction result returned by the chain.
     * @throws IOException          if the HTTP request fails.
     * @throws InterruptedException if the thread is interrupted while waiting for a semaphore permit.
     */
    public TransactionResult execute(ContractId contractId, String methodName, String methodArgs, boolean shouldHideArgs)
        throws IOException, InterruptedException {
        semaphore.acquire();
        try {
            String fromAddr;
            String toAddr;
            String publicKeyHex;
            synchronized (walletLock) {
                fromAddr = wallet.getAddress();
                toAddr = fromAddr;
                publicKeyHex = Utils.bytesToHex(wallet.getPublicKeyUncompressed());
            }
            int weilpodCounter = contractId.podCounter();
            long nonce = Utils.currentTimeMillis();

            TransactionHeader header = new TransactionHeader(
                nonce, publicKeyHex, fromAddr, toAddr, null, weilpodCounter, 0);

            Map<String, Object> args = new LinkedHashMap<>();
            args.put("contract_address", contractId.toString());
            args.put("contract_method", methodName);
            args.put("contract_input_bytes", methodArgs);
            args.put("should_hide_args", shouldHideArgs);

            Map<String, Object> userTxn = new TreeMap<>();
            userTxn.put("contract_address", contractId.toString());
            userTxn.put("contract_input_bytes", methodArgs);
            userTxn.put("contract_method", methodName);
            userTxn.put("should_hide_args", shouldHideArgs);
            userTxn.put("type", "SmartContractExecutor");

            Map<String, Object> payload = new TreeMap<>();
            payload.put("from_addr", fromAddr);
            payload.put("nonce", nonce);
            payload.put("to_addr", toAddr);
            payload.put("user_txn", userTxn);

            String canonicalJson = JSON.writeValueAsString(payload);
            String signature;
            synchronized (walletLock) {
                signature = wallet.sign(canonicalJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            }
            header.setSignature(signature);

            header.setCreationTime(Utils.currentTimeMillis());
            Verifier verifier = new Verifier();
            UserTransaction userTxnObj = new UserTransaction(
                "SmartContractExecutor", contractId, methodName, methodArgs, shouldHideArgs);
            TransactionPayload txn = new TransactionPayload(false, header, verifier, userTxnObj);
            SubmitTxnRequest req = new SubmitTxnRequest(txn);

            boolean nonBlocking = !shouldHideArgs;
            return PlatformApi.submitTransaction(req, httpClient, sentinelHost, nonBlocking);
        } finally {
            semaphore.release();
        }
    }

    /**
     * No-op close: Java 11+ {@link HttpClient} does not require explicit shutdown.
     * Provided for compatibility with try-with-resources blocks.
     */
    @Override
    public void close() {
        // HttpClient doesn't need explicit close in Java 11+
    }
}
