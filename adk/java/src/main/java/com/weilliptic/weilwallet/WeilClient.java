package com.weilliptic.weilwallet;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.weilliptic.weilwallet.api.*;
import com.weilliptic.weilwallet.transaction.OrgContext;
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
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import java.util.UUID;
import java.util.concurrent.Semaphore;
import java.util.concurrent.locks.ReentrantLock;

/**
 * High-level client for WeilChain applet methods. Holds an HTTP client, a signer Wallet, and a concurrency limiter.
 */
public class WeilClient implements AutoCloseable {

    private static final ObjectMapper JSON = new ObjectMapper();
    private static final String AUDIT_APPLET_SVC_NAME = "auditor::weil";

    /** WeilStream's publisher applet, as the Python SDK's client addresses it. */
    public static final String PUBLISHER_APPLET_SVC_NAME = "publisher::weil";

    /** Applet addresses resolved by service name, pinned to this wallet's pod. */
    private final Map<String, ContractId> appletIds = new java.util.concurrent.ConcurrentHashMap<>();

    private final Wallet wallet;
    private final String sentinelHost;
    private final HttpClient httpClient;
    private final Semaphore semaphore;
    private final ReentrantLock walletLock = new ReentrantLock();
    private volatile ContractId auditContractId;
    private boolean remoteSigning;
    private Map<String, Object> credentials;
    private JsonNode walletSourceJson;

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
     * Create a WeilClient from an Agent Registry API key.
     *
     * <p>The returned wallet is public-only; transaction signing is delegated to
     * Sentinel's {@code /sign_payload} endpoint.</p>
     */
    public static WeilClient fromApiKey(String apiKey) throws IOException, InterruptedException {
        return fromApiKey(apiKey, null, null);
    }

    public static WeilClient fromApiKey(String apiKey, Map<String, Object> credentials)
        throws IOException, InterruptedException {
        return fromApiKey(apiKey, credentials, null);
    }

    public static WeilClient fromApiKey(String apiKey, Map<String, Object> credentials, String sentinelHost)
        throws IOException, InterruptedException {
        String host = sentinelHost != null && !sentinelHost.isEmpty() ? sentinelHost : Constants.SENTINEL_HOST;
        HttpClient httpClient = TlsMode.httpClient();
        JsonNode walletJson = getAgentWalletJson(httpClient, host, apiKey, credentials);
        Wallet wallet = Wallet.fromWalletJson(walletJson, true);
        // The connection the wallet was fetched on is kept for the client's
        // calls, so the first of them needs no second TLS handshake.
        WeilClient client = new WeilClient(wallet, null, host, httpClient);
        client.remoteSigning = true;
        client.credentials = credentials;
        client.walletSourceJson = walletJson;
        return client;
    }

    public WeilClient(Wallet wallet) {
        this(wallet, null, null);
    }

    public WeilClient(Wallet wallet, String sentinelHost) {
        this(wallet, null, sentinelHost);
    }

    public WeilClient(Wallet wallet, Integer concurrency, String sentinelHost) {
        this(wallet, concurrency, sentinelHost, TlsMode.httpClient());
    }

    private WeilClient(Wallet wallet, Integer concurrency, String sentinelHost, HttpClient httpClient) {
        this.wallet = wallet;
        this.sentinelHost = sentinelHost != null && !sentinelHost.isEmpty() ? sentinelHost : Constants.SENTINEL_HOST;
        this.httpClient = httpClient;
        int conc = concurrency != null ? concurrency : Constants.DEFAULT_CONCURRENCY;
        this.semaphore = new Semaphore(conc);
        this.auditContractId = null;
        this.remoteSigning = false;
        this.credentials = null;
        this.walletSourceJson = null;
    }

    private static JsonNode getAgentWalletJson(
        HttpClient httpClient,
        String sentinelHost,
        String apiKey,
        Map<String, Object> credentials
    ) throws IOException, InterruptedException {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("identity", apiKey);
        payload.put("unmasked", false);
        if (credentials != null) {
            payload.put("credentials", credentials);
        }

        String url = sentinelHost.replaceAll("/$", "") + "/get_agent_wallet";
        String body = JSON.writeValueAsString(payload);
        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(url))
            .timeout(Duration.ofSeconds(60))
            .header("Content-Type", "application/json")
            .POST(HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8))
            .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        // Sentinel reports some failures as plain text rather than JSON, so the
        // body is only parsed once it is known to be a success, and an error
        // keeps its own wording instead of surfacing as a JSON parse error.
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            String message = response.body();
            try {
                JsonNode error = JSON.readTree(message);
                if (error.has("message")) {
                    message = error.get("message").asText();
                }
            } catch (IOException notJson) {
                // keep the raw body
            }
            throw new RuntimeException("agent wallet lookup failed: HTTP " + response.statusCode() + ": " + message);
        }
        JsonNode result;
        try {
            result = JSON.readTree(response.body());
        } catch (IOException notJson) {
            throw new RuntimeException("agent wallet lookup failed: HTTP " + response.statusCode()
                + ": " + response.body(), notJson);
        }

        if (result.isObject() && "wallet".equals(result.path("type").asText())) {
            return result;
        }
        if (result.isTextual()) {
            return JSON.readTree(result.asText());
        }
        throw new RuntimeException("agent wallet lookup failed: unexpected response type");
    }

    public static boolean walletHasExecutionPermission(String walletAddr, String svcName)
        throws IOException, InterruptedException {
        return walletHasExecutionPermission(walletAddr, svcName, Constants.SENTINEL_HOST);
    }

    public static boolean walletHasExecutionPermission(String walletAddr, String svcName, String sentinelHost)
        throws IOException, InterruptedException {
        return walletHasPurpose(walletAddr, svcName, "Execution", sentinelHost);
    }

    public static boolean walletHasManagementPermission(String walletAddr, String svcName)
        throws IOException, InterruptedException {
        return walletHasManagementPermission(walletAddr, svcName, Constants.SENTINEL_HOST);
    }

    public static boolean walletHasManagementPermission(String walletAddr, String svcName, String sentinelHost)
        throws IOException, InterruptedException {
        return walletHasPurpose(walletAddr, svcName, "Management", sentinelHost);
    }

    /**
     * Ask an organisation's identity applet whether a key holds {@code purpose}.
     *
     * <p>{@code svcName} names a subgroup within an organisation
     * ("engg.weil"), not an applet. The organisation is the part after the last
     * dot, and its identity applet is "identity::&lt;org&gt;"; the subgroup
     * prefixes the key the applet is asked about, since keys are enrolled there
     * as "&lt;subgroup&gt;_&lt;address&gt;". Resolving {@code svcName} itself as
     * an applet is what makes Sentinel answer "Not able to retrieve Applet
     * address for Service Name".</p>
     *
     * <p>Org-level keys span every subgroup, so the bare address is asked
     * first and the subgroup key second, as the Python SDK does.</p>
     */
    private static boolean walletHasPurpose(
        String walletAddr, String svcName, String purpose, String sentinelHost)
        throws IOException, InterruptedException {
        String host = sentinelHost != null && !sentinelHost.isEmpty() ? sentinelHost : Constants.SENTINEL_HOST;
        HttpClient httpClient = TlsMode.httpClient();

        int dot = svcName.lastIndexOf('.');
        String org = dot >= 0 ? svcName.substring(dot + 1) : svcName;
        // The identity applet is looked up by name alone: it belongs to the
        // organisation, not to the wallet being asked about.
        ContractId appletId = getAppletIdForName(
            httpClient, host, "identity::" + (org.isEmpty() ? svcName : org), null);

        for (String key : purposeKeys(walletAddr, svcName)) {
            if (keyHasPurpose(httpClient, host, appletId, walletAddr, key, purpose)) {
                return true;
            }
        }
        return false;
    }

    /** The keys a wallet may be enrolled under: org-level first, then subgroup. */
    private static List<String> purposeKeys(String walletAddr, String svcName) {
        int dot = svcName.lastIndexOf('.');
        String subgroup = dot >= 0 ? svcName.substring(0, dot) : "";
        return subgroup.isEmpty()
            ? List.of(walletAddr)
            : List.of(walletAddr, subgroup + "_" + walletAddr);
    }

    private static boolean keyHasPurpose(
        HttpClient httpClient, String host, ContractId appletId,
        String walletAddr, String key, String purpose)
        throws IOException, InterruptedException {
        String methodArgs = JSON.writeValueAsString(Map.of(
            "key", key,
            "purpose", purpose
        ));

        TransactionHeader header = new TransactionHeader(
            Utils.currentTimeMillis(),
            "",
            walletAddr,
            walletAddr,
            // An empty string, not null: this query carries no signature, and
            // the node's header field is a string. A JSON null is not the same
            // value, and the submit never comes back.
            "",
            appletId.podCounter(),
            0,
            UUID.randomUUID().toString(),
            null
        );
        TransactionPayload txn = new TransactionPayload(
            false,
            header,
            new Verifier(),
            new UserTransaction("SmartContractExecutor", appletId, "key_has_purpose", methodArgs, true)
        );
        TransactionResult resp = PlatformApi.submitTransaction(
            new SubmitTxnRequest(txn),
            httpClient,
            host,
            false
        );
        JsonNode result = JSON.readTree(resp.getTxnResult());
        if (!result.has("Ok")) {
            return false;
        }
        JsonNode ok = result.get("Ok");
        if (ok.isBoolean()) {
            return ok.asBoolean();
        }
        if (ok.isTextual()) {
            return "true".equals(ok.asText());
        }
        return false;
    }

    /**
     * Resolve a service name (e.g. {@code "identity::weil"}) to its ContractId.
     *
     * <p>Exposed for the ADK integration, which addresses applets by name.</p>
     */
    public static ContractId resolveApplet(String svcName, String sentinelHost, String walletAddr)
        throws IOException, InterruptedException {
        String host = sentinelHost != null ? sentinelHost : Constants.SENTINEL_HOST;
        HttpClient httpClient = TlsMode.httpClient();
        return getAppletIdForName(httpClient, host, svcName, walletAddr);
    }

    private static ContractId getAppletIdForName(
        HttpClient httpClient,
        String sentinelHost,
        String svcName,
        String walletAddress
    ) throws IOException, InterruptedException {
        Map<String, String> payload = walletAddress != null && !walletAddress.isEmpty()
            ? Map.of("svc_name", svcName, "wallet_address", walletAddress)
            : Map.of("svc_name", svcName);
        String body = JSON.writeValueAsString(payload);
        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(sentinelHost.replaceAll("/$", "") + "/get_applet_address"))
            .timeout(Duration.ofSeconds(60))
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
                return ContractId.of(address);
            }
        }
        throw new RuntimeException("get_applet_address failed: " + data.getOrDefault("Err", data));
    }

    public Map<String, String> buildAuthHeaders() {
        return buildAuthHeaders(null);
    }

    /**
     * Build signed auth headers, optionally bound to a request body.
     *
     * <p>Signs locally, or through Sentinel for API-key wallets that hold no
     * private key. Either way the headers carry two addresses: the 64-hex
     * {@code sha256(public key)} in {@code X-Wallet-Address}, which a server
     * can re-derive from the signature to prove key ownership, and the
     * canonical sentinel-minted address inside the signed message, which is
     * what identity applets are keyed on and cannot be derived from a key.
     * See {@link WeilAuth#canonicalAddress}.</p>
     *
     * @param body request body to bind, or null for no binding
     */
    public Map<String, String> buildAuthHeaders(Object body) {
        if (!remoteSigning) {
            walletLock.lock();
            try {
                return WeilAuth.buildHeaders(wallet, body);
            } finally {
                walletLock.unlock();
            }
        }
        try {
            String timestamp = String.valueOf(System.currentTimeMillis() / 1000L);
            Map<String, String> message = new TreeMap<>();
            message.put("timestamp", timestamp);
            String canonical;
            byte[] publicKey;
            walletLock.lock();
            try {
                canonical = wallet.getAddress();
                publicKey = wallet.getPublicKeyUncompressed();
            } finally {
                walletLock.unlock();
            }
            message.put("address", canonical);
            String bodyHash = null;
            String nonce = null;
            if (body != null) {
                bodyHash = WeilAuth.bodySha256(body);
                nonce = Utils.bytesToHex(randomBytes(16));
                message.put("body_sha256", bodyHash);
                message.put("nonce", nonce);
            }
            String json = JSON.writeValueAsString(message);
            Map<String, Object> payload =
                JSON.readValue(json, new TypeReference<Map<String, Object>>() {});
            String signature = signMessage(payload);

            Map<String, String> headers = new TreeMap<>();
            headers.put("X-Wallet-Address", Utils.bytesToHex(Utils.hashSha256(publicKey)));
            headers.put("X-Signature", signature);
            headers.put("X-Message", json);
            headers.put("X-Timestamp", timestamp);
            if (bodyHash != null) {
                headers.put("X-Body-SHA256", bodyHash);
                headers.put("X-Nonce", nonce);
            }
            return headers;
        } catch (Exception e) {
            throw new RuntimeException("failed to build Weil auth headers", e);
        }
    }

    private static byte[] randomBytes(int count) {
        byte[] bytes = new byte[count];
        new java.security.SecureRandom().nextBytes(bytes);
        return bytes;
    }

    public String walletAddress() {
        walletLock.lock();
        try {
            return wallet.getAddress();
        } finally {
            walletLock.unlock();
        }
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
        return execute(contractId, "audit", auditArgs(log), false);
    }

    // ── WeilStream ───────────────────────────────────────────────────────────

    /**
     * Create a WeilStream topic through {@code publisher::weil}, signed by this
     * wallet, which becomes the topic's publisher of record.
     *
     * <p>Mirrors the Python SDK's {@code WeilClient.create_topic}. Waits for the
     * applet to run.</p>
     */
    public TransactionResult createTopic(String topic, String description, long retentionInBytes)
        throws IOException, InterruptedException {
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("topic", topic);
        args.put("description", description);
        args.put("retention_in_bytes", retentionInBytes);
        return execute(appletId(PUBLISHER_APPLET_SVC_NAME), "create_topic",
            JSON.writeValueAsString(args), false, false);
    }

    /**
     * Publish {@code payload} to {@code topic} through {@code publisher::weil},
     * signed by this wallet. Waits for the applet to run.
     *
     * <p>Mirrors the Python SDK's {@code WeilClient.publish}.</p>
     */
    public TransactionResult publish(String topic, String payload)
        throws IOException, InterruptedException {
        return publish(PUBLISHER_APPLET_SVC_NAME, topic, payload, false);
    }

    /**
     * Publish through the publisher applet named {@code publisherSvcName}, such
     * as an organisation's own {@code publisher::<org>}.
     *
     * @param isNonBlocking true to return once submitted, without waiting for
     *                      the applet to run
     */
    public TransactionResult publish(
        String publisherSvcName, String topic, String payload, boolean isNonBlocking)
        throws IOException, InterruptedException {
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("topic", topic);
        args.put("payload", payload);
        return execute(appletId(publisherSvcName), "publish",
            JSON.writeValueAsString(args), false, isNonBlocking);
    }

    /**
     * An applet's address by service name, resolved once and cached. Sends
     * this wallet's address, so Sentinel answers from the wallet's home pod.
     */
    public ContractId appletId(String svcName) throws IOException, InterruptedException {
        ContractId cached = appletIds.get(svcName);
        if (cached != null) {
            return cached;
        }
        ContractId resolved = getAppletIdForName(httpClient, sentinelHost, svcName, walletAddress());
        appletIds.put(svcName, resolved);
        return resolved;
    }

    // ── Auditor receipts ─────────────────────────────────────────────────────

    /** Where {@link #persistReceipt} stored a receipt. */
    public record ReceiptRef(long location, String receiptId) {
        /** The storage cloud, from the high 16 bits of {@code location}. */
        public int cloud() {
            return (int) ((location >>> 16) & 0xFFFF);
        }

        /** The bucket, from the low 16 bits of {@code location}. */
        public int bucket() {
            return (int) (location & 0xFFFF);
        }
    }

    /**
     * Store {@code content} as a receipt through the auditor's
     * {@code validate_and_persist_receipt}, which checks this wallet's org
     * membership and keys the receipt by the SHA-256 of the content.
     *
     * <p>Mirrors the Go SDK's {@code PersistReceipt}. Waits for the applet.</p>
     */
    public ReceiptRef persistReceipt(String content) throws IOException, InterruptedException {
        OrgInfo org = org();
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("org", org != null ? org.name() : null);
        args.put("subgroup", org != null ? org.subgroup() : null);
        args.put("hash", Utils.bytesToHex(Utils.hashSha256(content.getBytes(StandardCharsets.UTF_8))));
        args.put("content", content);
        TransactionResult result = execute(getAuditContractId(), "validate_and_persist_receipt",
            JSON.writeValueAsString(args), false, false);
        JsonNode ok = unwrapResult(result.getTxnResult(), "validate_and_persist_receipt");
        if (ok == null || !ok.isObject()) {
            throw new RuntimeException("validate_and_persist_receipt returned no receipt: "
                + result.getTxnResult());
        }
        return new ReceiptRef(ok.path("location").asLong(), ok.path("receipt_id").asText(null));
    }

    /**
     * An applet result with its envelope taken off, as the Python SDK does it:
     * {@code {"Ok": ...}} unwrapped, then, when the value is a string, parsed
     * once more, since the platform re-serializes a callee's return value to
     * JSON text. A string that is not JSON is returned as is.
     *
     * @throws RuntimeException on {@code {"Err": ...}}
     */
    public static JsonNode unwrapResult(String txnResult, String method) {
        if (txnResult == null || txnResult.isEmpty()) {
            return null;
        }
        JsonNode node;
        try {
            node = JSON.readTree(txnResult);
        } catch (IOException e) {
            throw new RuntimeException("failed to parse " + method + " response: " + txnResult, e);
        }
        if (node != null && node.isObject()) {
            if (node.has("Err")) {
                throw new RuntimeException(method + " returned an error: " + node.get("Err"));
            }
            if (node.has("Ok")) {
                node = node.get("Ok");
            }
        }
        if (node != null && node.isTextual()) {
            try {
                return JSON.readTree(node.asText());
            } catch (IOException notJson) {
                return node;
            }
        }
        return node;
    }

    /** The active organisation membership, or null when the wallet has none. */
    public OrgInfo org() {
        walletLock.lock();
        try {
            return wallet.org();
        } finally {
            walletLock.unlock();
        }
    }

    /**
     * The auditor applet's arguments: the entry plus the organisation it was
     * written under.
     *
     * <p>{@code org} and {@code subgroup} are passed as method arguments, not
     * just carried in the signed header, so the applet can attribute the entry
     * without re-deriving it. Both are always present, null when the wallet has
     * no active org.</p>
     */
    public String auditArgs(String log) throws IOException {
        OrgInfo org = org();
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("log", log != null ? log : "");
        args.put("org", org != null ? org.name() : null);
        args.put("subgroup", org != null ? org.subgroup() : null);
        return JSON.writeValueAsString(args);
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

            OrgContext org;

            walletLock.lock();
            try {
                fromAddr = wallet.getAddress();
                toAddr = fromAddr;
                publicKeyHex = Utils.bytesToHex(wallet.getPublicKeyUncompressed());
                // Snapshot the wallet's active org under the same lock as the
                // address it is paired with.
                org = OrgContext.fromOrgInfo(wallet.org());
            } finally {
                walletLock.unlock();
            }

            int weilpodCounter = contractId.podCounter();
            long nonce = Utils.currentTimeMillis();
            String salt = UUID.randomUUID().toString();

            TransactionHeader header = new TransactionHeader(
                nonce, publicKeyHex, fromAddr, toAddr, null, weilpodCounter, 0, salt, org);

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

            // `org` is included **only when present**, mirroring the node's
            // compute_verify_digest_for_execute. This conditionality preserves
            // compatibility: a wallet with no active org produces exactly the
            // bytes it did before org existed, so already-signed transactions
            // still verify. Putting a null org in unconditionally would change
            // the digest for every org-less transaction at once.
            //
            // The enclosing TreeMap places the key correctly regardless of
            // where it is inserted here.
            if (org != null) {
                payload.put("org", org.toMap());
            }

            String canonicalJson = JSON.writeValueAsString(payload);

            if (remoteSigning) {
                signature = signMessage(payload);
            } else {
                walletLock.lock();
                try {
                    signature = wallet.sign(canonicalJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
                } finally {
                    walletLock.unlock();
                }
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

    @Override
    public void close() {
        // HttpClient doesn't need explicit close in Java 11+
    }

    /**
     * Sign a canonical payload via Sentinel remote signing.
     */
    public String signMessage(Map<String, Object> payload) throws IOException, InterruptedException {
        if (!remoteSigning) {
            throw new IllegalStateException("signMessage requires an API-key client");
        }
        if (walletSourceJson == null) {
            throw new IllegalStateException("remote signing credentials are unavailable");
        }

        Map<String, Object> body = new LinkedHashMap<>();
        body.put("payload", payload);
        body.put("wallet", walletSourceJson);
        if (credentials != null) {
            body.put("credentials", credentials);
        }

        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(sentinelHost.replaceAll("/$", "") + "/sign_payload"))
            .timeout(Duration.ofSeconds(60))
            .header("Content-Type", "application/json")
            .POST(HttpRequest.BodyPublishers.ofString(JSON.writeValueAsString(body), StandardCharsets.UTF_8))
            .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw new RuntimeException("remote signing failed: HTTP " + response.statusCode() + " " + response.body());
        }

        JsonNode result = JSON.readTree(response.body());
        if (result.isObject() && result.has("signature")) {
            return result.get("signature").asText();
        }
        if (result.isTextual()) {
            return result.asText();
        }
        throw new RuntimeException("remote signing failed: response missing signature");
    }
}
