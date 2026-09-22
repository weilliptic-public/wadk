package com.weilliptic.weilwallet;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.JsonNode;
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
import java.util.Optional;
import java.util.TreeMap;
import java.util.UUID;
import java.util.concurrent.Semaphore;
import java.util.concurrent.locks.ReentrantLock;

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
    private static final String AUDIT_APPLET_SVC_NAME = "auditor::weil";

    private final Wallet wallet;
    private final String sentinelHost;
    private final HttpClient httpClient;
    private final Semaphore semaphore;
    private final ReentrantLock walletLock = new ReentrantLock();
    private volatile ContractId auditContractId;

    /**
     * Create a WeilClient by loading a wallet directly from a wallet.wc file.
     *
     * @param path Path to the wallet.wc file.
     * @return A new WeilClient bound to the loaded wallet.
     * @throws IOException If the file cannot be read or parsed.
     */
    public static WeilClient fromWalletFile(String path) throws IOException {
        return new WeilClient(Wallet.fromWalletFile(path));
    }

    /**
     * Create a WeilClient with the given wallet, default sentinel host,
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
     * Resolve and cache the audit applet contract address from the Sentinel API.
     *
     * <p>Sends the caller's own {@code wallet_address} alongside {@code svc_name} so Sentinel pins
     * resolution to that wallet's home weilpod (derived server-side from the pod counter embedded
     * in the address) instead of falling back to a random pod in its region. Without this,
     * {@code validate_and_persist_receipt} can land on a pod whose local {@code identity::<org>}
     * copy never saw this wallet's membership.</p>
     */
    private synchronized ContractId getAuditContractId() throws IOException, InterruptedException {
        if (auditContractId != null) {
            return auditContractId;
        }
        String walletAddress;
        walletLock.lock();
        try {
            walletAddress = wallet.getAddress();
        } finally {
            walletLock.unlock();
        }
        String url = sentinelHost.replaceAll("/$", "") + "/get_applet_address";
        String body = JSON.writeValueAsString(
            Map.of("svc_name", AUDIT_APPLET_SVC_NAME, "wallet_address", walletAddress));
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

    // ── Multi-account management ──────────────────────────────────────────

    /**
     * Switch the active account in the wallet.
     *
     * <p>Thread-safe: acquires the wallet lock before mutating the wallet.</p>
     *
     * <p>Also drops the cached audit contract id — it's pinned to whichever account's home pod
     * resolved it (see {@link #getAuditContractId()}), so a stale entry from the previous account
     * must not leak into calls made under the new one. The wallet lock is released before the
     * monitor guarding the cache is taken, because {@code getAuditContractId} holds them in the
     * opposite order and holding both here would risk a deadlock.</p>
     *
     * @param selected The account selector (e.g. {@code SelectedAccount.External(1)}).
     */
    public void setAccount(SelectedAccount selected) {
        walletLock.lock();
        try {
            wallet.setIndex(selected);
        } finally {
            walletLock.unlock();
        }
        synchronized (this) {
            auditContractId = null;
        }
    }

    /**
     * Read back the receipt content currently persisted for {@code commitHash}, or an empty
     * {@link Optional} if nothing has been persisted for it yet.
     *
     * <p>Used by the same-turn-commit-gap amend path to fetch the payload an agent-run mid-turn
     * commit already shipped — with empty prompts/usage, since that commit landed before Stop had
     * computed them — so it can be merged and re-persisted under the same commit hash.</p>
     */
    public Optional<String> getReceiptForCommit(String commitHash) throws IOException, InterruptedException {
        ContractId contractId = getAuditContractId();
        String methodArgs = JSON.writeValueAsString(Map.of("commit_hash", commitHash));
        TransactionResult resp = execute(contractId, "get_receipt_for_commit", methodArgs, false, false);
        return parseGetReceiptForCommitResult(resp.getTxnResult());
    }

    /**
     * Decode the {@code txn_result} of a {@code get_receipt_for_commit} call into the receipt
     * content, or an empty {@link Optional} when nothing is persisted for the commit.
     *
     * <p>The value is double-wrapped: the platform puts every contract call's result in an
     * {@code {"Ok": ...}}/{@code {"Err": ...}} envelope, and "Ok"'s value is itself the callee's
     * return value <em>re-serialized to a JSON string</em> rather than embedded directly. So this
     * needs two decode passes: unwrap the envelope, then parse the resulting string to reach the
     * actual optional receipt.</p>
     */
    static Optional<String> parseGetReceiptForCommitResult(String txnResult) {
        JsonNode envelope;
        try {
            envelope = JSON.readTree(txnResult);
        } catch (JsonProcessingException e) {
            throw new RuntimeException("failed to parse get_receipt_for_commit response: " + e.getMessage(), e);
        }
        if (envelope == null || envelope.isMissingNode()) {
            throw new RuntimeException("failed to parse get_receipt_for_commit response: empty result");
        }
        if (envelope.has("Err")) {
            throw new RuntimeException("get_receipt_for_commit returned an error: " + envelope.get("Err"));
        }

        JsonNode okValue = envelope.has("Ok") ? envelope.get("Ok") : envelope;
        if (okValue.isNull()) {
            return Optional.empty();
        }

        JsonNode inner = okValue;
        if (okValue.isTextual()) {
            try {
                inner = JSON.readTree(okValue.asText());
            } catch (JsonProcessingException e) {
                throw new RuntimeException(
                    "failed to parse get_receipt_for_commit inner value: " + e.getMessage(), e);
            }
        }

        if (inner == null || inner.isNull() || inner.isMissingNode()) {
            return Optional.empty();
        }
        if (inner.isTextual()) {
            return Optional.of(inner.asText());
        }
        throw new RuntimeException("unexpected get_receipt_for_commit payload shape: " + inner);
    }

    /**
     * Return the number of HD-derived accounts.
     *
     * <p>Thread-safe: acquires the wallet lock before reading the wallet.</p>
     */
    public int derivedAccountCount() {
        walletLock.lock();
        try {
            return wallet.derivedAccountCount();
        } finally {
            walletLock.unlock();
        }
    }

    /**
     * Return the number of externally imported accounts.
     *
     * <p>Thread-safe: acquires the wallet lock before reading the wallet.</p>
     */
    public int externalAccountCount() {
        walletLock.lock();
        try {
            return wallet.externalAccountCount();
        } finally {
            walletLock.unlock();
        }
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
        return execute(contractId, methodName, methodArgs, shouldHideArgs, !shouldHideArgs);
    }

    /**
     * Execute a contract method and return the transaction result, choosing the blocking mode
     * explicitly. Callers that need the contract's return value must pass
     * {@code isNonBlocking = false}, otherwise the result comes back before the transaction has
     * been applied and {@code txnResult} is empty.
     */
    public TransactionResult execute(ContractId contractId, String methodName, String methodArgs,
                                     boolean shouldHideArgs, boolean isNonBlocking)
        throws IOException, InterruptedException {
        semaphore.acquire();
        try {
            String fromAddr;
            String toAddr;
            String publicKeyHex;
            String signature;

            walletLock.lock();
            try {
                fromAddr = wallet.getAddress();
                toAddr = fromAddr;
                publicKeyHex = Utils.bytesToHex(wallet.getPublicKeyUncompressed());
            } finally {
                walletLock.unlock();
            }

            int weilpodCounter = contractId.podCounter();
            long nonce = Utils.currentTimeMillis();
            String salt = UUID.randomUUID().toString();

            TransactionHeader header = new TransactionHeader(
                nonce, publicKeyHex, fromAddr, toAddr, null, weilpodCounter, 0, salt);

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
            payload.put("salt", salt);
            payload.put("to_addr", toAddr);
            payload.put("user_txn", userTxn);

            String canonicalJson = JSON.writeValueAsString(payload);

            walletLock.lock();
            try {
                signature = wallet.sign(canonicalJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            } finally {
                walletLock.unlock();
            }

            header.setSignature(signature);

            header.setCreationTime(Utils.currentTimeMillis());
            Verifier verifier = new Verifier();
            UserTransaction userTxnObj = new UserTransaction(
                "SmartContractExecutor", contractId, methodName, methodArgs, shouldHideArgs);
            TransactionPayload txn = new TransactionPayload(false, header, verifier, userTxnObj);
            SubmitTxnRequest req = new SubmitTxnRequest(txn);

            return PlatformApi.submitTransaction(req, httpClient, sentinelHost, isNonBlocking);
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
