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
 * High-level client for WeilChain applet methods. Holds an HTTP client, a signer Wallet, and a concurrency limiter.
 */
public class WeilClient implements AutoCloseable {

    private static final ObjectMapper JSON = new ObjectMapper();
    private static final String AUDIT_APPLET_SVC_NAME = "auditor";

    private final Wallet wallet;
    private final String sentinelHost;
    private final HttpClient httpClient;
    private final Semaphore semaphore;
    private volatile ContractId auditContractId;

    public WeilClient(Wallet wallet) {
        this(wallet, null, null);
    }

    public WeilClient(Wallet wallet, String sentinelHost) {
        this(wallet, null, sentinelHost);
    }

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
     */
    public TransactionResult execute(ContractId contractId, String methodName, String methodArgs, boolean shouldHideArgs)
        throws IOException, InterruptedException {
        semaphore.acquire();
        try {
            String fromAddr = Utils.getAddressFromPublicKey(wallet.getECKey());
            String toAddr = fromAddr;
            String publicKeyHex = Utils.bytesToHex(wallet.getPublicKeyUncompressed());
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
            String signature = wallet.sign(canonicalJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
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

    @Override
    public void close() {
        // HttpClient doesn't need explicit close in Java 11+
    }
}
