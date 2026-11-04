package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.google.adk.a2a.executor.AgentExecutor;
import com.google.adk.a2a.executor.AgentExecutorConfig;
import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.InvocationContext;
import com.google.adk.artifacts.InMemoryArtifactService;
import com.google.adk.plugins.Plugin;
import com.google.adk.apps.App;
import com.google.adk.sessions.InMemorySessionService;
import com.google.genai.types.Content;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import com.weilliptic.weilwallet.WeilAuth;

import io.a2a.server.ServerCallContext;
import io.a2a.server.auth.User;
import io.a2a.server.events.InMemoryQueueManager;
import io.a2a.server.requesthandlers.DefaultRequestHandler;
import io.a2a.server.tasks.BasePushNotificationSender;
import io.a2a.server.tasks.InMemoryPushNotificationConfigStore;
import io.a2a.server.tasks.InMemoryTaskStore;
import io.a2a.spec.AgentCapabilities;
import io.a2a.spec.AgentCard;
import io.a2a.spec.AgentExtension;
import io.a2a.spec.AgentInterface;
import io.a2a.spec.AgentSkill;
import io.a2a.spec.CancelTaskRequest;
import io.a2a.spec.EventKind;
import io.a2a.spec.GetTaskRequest;
import io.a2a.spec.InternalError;
import io.a2a.spec.InvalidParamsError;
import io.a2a.spec.InvalidRequestError;
import io.a2a.spec.JSONParseError;
import io.a2a.spec.JSONRPCErrorResponse;
import io.a2a.spec.Message;
import io.a2a.spec.MessageSendParams;
import io.a2a.spec.MethodNotFoundError;
import io.a2a.spec.SendMessageRequest;
import io.a2a.spec.SendMessageResponse;
import io.a2a.spec.SendStreamingMessageRequest;
import io.a2a.spec.Task;
import io.a2a.spec.TaskNotFoundError;
import io.a2a.spec.TaskResubscriptionRequest;
import io.a2a.spec.TransportProtocol;
import io.a2a.spec.UnsupportedOperationError;
import io.a2a.transport.jsonrpc.handler.JSONRPCHandler;
import io.a2a.util.Utils;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Serves an ADK agent to other agents over the A2A protocol, signed by Weil
 * wallets.
 *
 * <p>The agent speaks standard A2A (protocol 0.3, JSON-RPC transport), so any
 * A2A client can reach it: a Java agent through {@link WeilA2A}, a Python one
 * through ADK's {@code RemoteA2aAgent}, or any other. The server offers:</p>
 *
 * <ul>
 *   <li>{@code GET /.well-known/agent-card.json}: the agent card, naming the
 *       agent's wallet and the permission callers need;</li>
 *   <li>{@code POST /}: JSON-RPC {@code message/send}, {@code tasks/get} and
 *       {@code tasks/cancel}. Streaming is not offered, and the card says
 *       so.</li>
 * </ul>
 *
 * <p>Every request must be signed by the calling agent's wallet, with the
 * signature bound to the A2A message ({@code params.message}) exactly as the
 * Python SDK binds it. On each one the server, in order:</p>
 *
 * <ol>
 *   <li>verifies the signature and the body binding — 401 if either fails;</li>
 *   <li>checks the caller's wallet for Execution or Management purpose on
 *       the configured service — 403 if it holds neither;</li>
 *   <li>runs the agent and returns its answer.</li>
 * </ol>
 *
 * <p>A rejected request never reaches the agent, so no model tokens are spent
 * on a caller that was not allowed to ask. A conversation ({@code contextId})
 * belongs to the wallet that started it: another wallet cannot continue it or
 * read its tasks.</p>
 *
 * <pre>{@code
 * WeilAgentServer server = WeilAgentServer.builder(agent, identity)
 *     .port(8001)                    // else PORT, else 8001
 *     .requirePermission("engg.weil")
 *     .plugins(List.of(weilPlugin))
 *     .build();
 * server.start();
 * }</pre>
 */
public final class WeilAgentServer {

    private static final ObjectMapper JSON = new ObjectMapper();

    /** Where A2A clients look for the agent card (protocol 0.3). */
    public static final String AGENT_CARD_PATH = "/.well-known/agent-card.json";

    /** The card's path before protocol 0.3, still asked for by older clients. */
    public static final String LEGACY_AGENT_CARD_PATH = "/.well-known/agent.json";

    /** Identifies the Weil signing scheme in the agent card's extensions. */
    public static final String WEIL_EXTENSION_URI = "https://weilliptic.ai/a2a/extensions/wallet-auth/v1";

    /** Key under which the verified {@link Caller} sits in the A2A call context's state. */
    public static final String CALLER_STATE_KEY = "weil.caller";

    /**
     * Key under which the verified caller's canonical address reaches the
     * agent run, inside the run config's {@code a2a_metadata}. The server
     * overwrites whatever a client sent under it, so a tool gate can trust it;
     * see {@link #callerWallet}.
     */
    public static final String CALLER_METADATA_KEY = "weil.caller_wallet";

    /** Where ADK's A2A executor puts a request's metadata in the run config. */
    private static final String A2A_METADATA = "a2a_metadata";

    /** The A2A protocol version this server implements. */
    private static final String A2A_PROTOCOL_VERSION = "0.3.0";

    /** The a2a SDK's own mapper, which knows its types' JSON shapes. */
    private static final ObjectMapper A2A_JSON = Utils.OBJECT_MAPPER;

    /** Decides whether a verified caller may proceed, beyond the applet check. */
    @FunctionalInterface
    public interface Authorizer {
        /** @return null to allow, or a reason to refuse */
        String refuse(Caller caller);
    }

    /** A caller whose signature this server verified. */
    public static final class Caller {
        private final String address;
        private final String canonicalAddress;

        Caller(String address, String canonicalAddress) {
            this.address = address;
            this.canonicalAddress = canonicalAddress;
        }

        /** The verifiable 64-hex form: sha256 of the caller's public key. */
        public String address() {
            return address;
        }

        /**
         * The canonical 72-hex address, when the caller signed one.
         *
         * <p>This is what identity applets are keyed on. Falls back to
         * {@link #address()} for callers that do not send it.</p>
         */
        public String canonicalAddress() {
            return canonicalAddress != null ? canonicalAddress : address;
        }

        @Override
        public String toString() {
            return canonicalAddress();
        }
    }

    private final BaseAgent agent;
    private final WeilIdentity identity;
    private final String appName;
    private final int port;
    private final String bindAddress;
    private final String requiredService;
    private final String sentinelHost;
    private final Authorizer authorizer;
    /** Every header {@code buildAuthHeaders} sets; all four are needed to verify. */
    private static final List<String> REQUIRED_AUTH_HEADERS =
        List.of("x-wallet-address", "x-signature", "x-message", "x-timestamp");

    private final List<Plugin> plugins;
    /** Writes refusal records off the request thread. */
    private final Dispatcher auditQueue;
    private final boolean auditRefusals;
    private final boolean publishPermissionChecks;

    private final String publicUrl;
    private final AgentCard agentCard;
    private final InMemoryTaskStore taskStore;
    private final JSONRPCHandler rpc;
    private final ExecutorService workers;
    /** The wallet that started each conversation; no other wallet may use it. */
    private final Map<String, String> contextOwners = new ConcurrentHashMap<>();

    private HttpServer http;

    private WeilAgentServer(Builder b) {
        this.agent = b.agent;
        this.identity = b.identity;
        this.appName = b.appName != null ? b.appName
            : b.app != null ? b.app.name() : b.agent.name();
        this.port = b.port;
        this.bindAddress = b.bindAddress != null ? b.bindAddress
            : envOr("WEIL_A2A_BIND", "127.0.0.1");
        this.requiredService = b.requiredService;
        this.sentinelHost = b.sentinelHost;
        this.authorizer = b.authorizer;
        this.plugins = List.copyOf(b.plugins);
        this.auditRefusals = b.auditRefusals;
        this.publishPermissionChecks = b.publishPermissionChecks;
        // Share the plugin's dispatcher when there is one, so refusal records
        // and hook records are delivered the same way; else inline, so none
        // is lost.
        this.auditQueue = b.plugins.stream()
            .filter(WeilPlugin.class::isInstance)
            .map(p -> ((WeilPlugin) p).dispatcher())
            .findFirst()
            .orElseGet(Dispatcher::inline);
        // An App keeps its own settings (context caching, event compaction,
        // resumability) and plugins, with these plugins put first.
        App app = b.app == null ? null : Weil.withPlugins(b.app, appName, plugins);

        String url = b.publicUrl != null ? b.publicUrl : System.getenv("WEIL_A2A_PUBLIC_URL");
        this.publicUrl = url != null && !url.isBlank() ? url.trim() : localUrl(bindAddress, port);
        this.agentCard = buildAgentCard();
        this.workers = Executors.newCachedThreadPool();
        this.taskStore = new InMemoryTaskStore();
        InMemoryPushNotificationConfigStore pushConfigs = new InMemoryPushNotificationConfigStore();
        AgentExecutor.Builder executorBuilder = new AgentExecutor.Builder()
            .sessionService(new InMemorySessionService())
            .artifactService(new InMemoryArtifactService())
            .agentExecutorConfig(AgentExecutorConfig.builder().build());
        AgentExecutor executor = (app != null
            ? executorBuilder.app(app)
            : executorBuilder.agent(agent).appName(appName).plugins(plugins))
            .build();
        DefaultRequestHandler handler = new DefaultRequestHandler(
            executor,
            taskStore,
            new InMemoryQueueManager(taskStore),
            pushConfigs,
            new BasePushNotificationSender(pushConfigs),
            workers);
        setBlockingTimeouts(handler, b.agentTimeoutSeconds);
        this.rpc = new JSONRPCHandler(agentCard, handler, workers);
    }

    public static Builder builder(BaseAgent agent, WeilIdentity identity) {
        return new Builder(agent, null, identity);
    }

    /**
     * Serve an ADK {@link App}: its root agent, with the App's own settings and
     * plugins kept, and the builder's plugins run before them.
     */
    public static Builder builder(App app, WeilIdentity identity) {
        return new Builder(app.rootAgent(), app, identity);
    }

    public int port() {
        return port;
    }

    /** The agent's own wallet address, which callers can pin. */
    public String walletAddress() {
        return identity.address();
    }

    /** The A2A agent card this server publishes. */
    public AgentCard agentCard() {
        return agentCard;
    }

    public void start() throws IOException {
        http = HttpServer.create(new InetSocketAddress(bindAddress, port), 0);
        http.createContext("/", this::handleA2A);
        http.createContext(AGENT_CARD_PATH, this::handleAgentCard);
        http.createContext(LEGACY_AGENT_CARD_PATH, this::handleAgentCard);
        http.setExecutor(Executors.newCachedThreadPool());
        http.start();
        System.out.println(agent.name() + " serving A2A at " + publicUrl);
        System.out.println("  agent card: " + publicUrl.replaceAll("/$", "") + AGENT_CARD_PATH);
        System.out.println("  wallet: " + identity.address());
        if (isWildcard(bindAddress) && publicUrl.equals(localUrl(bindAddress, port))) {
            System.out.println("  warning: listening on " + bindAddress + ", but the card names "
                + publicUrl + ", which callers on other hosts cannot reach; set WEIL_A2A_PUBLIC_URL");
        }
        if (requiredService != null) {
            System.out.println("  callers must hold Execution or Management on: " + requiredService);
        } else {
            System.out.println("  no agent permission check: any verified signature may call");
        }
    }

    /**
     * The port to serve on: {@code PORT} from the environment, as containers
     * and cloud platforms set it, else {@code fallback}.
     */
    private static int portFromEnv(int fallback) {
        String port = System.getenv("PORT");
        if (port == null || port.isBlank()) {
            return fallback;
        }
        try {
            return Integer.parseInt(port.trim());
        } catch (NumberFormatException e) {
            throw new IllegalStateException("PORT is \"" + port + "\"; want a port number", e);
        }
    }

    /** The URL the card names when none is given: the bind address, or loopback for all interfaces. */
    private static String localUrl(String bindAddress, int port) {
        String host = isWildcard(bindAddress) ? "127.0.0.1" : bindAddress;
        if (host.contains(":") && !host.startsWith("[")) {
            host = "[" + host + "]";
        }
        return "http://" + host + ":" + port + "/";
    }

    private static boolean isWildcard(String address) {
        return address.equals("0.0.0.0") || address.equals("::") || address.equals("[::]");
    }

    private static String envOr(String name, String fallback) {
        String value = System.getenv(name);
        return value == null || value.isBlank() ? fallback : value.trim();
    }

    public void stop(int delaySeconds) {
        if (http != null) {
            http.stop(delaySeconds);
        }
        workers.shutdown();
    }

    // ── A2A ──────────────────────────────────────────────────────────────────

    /**
     * How long a blocking {@code message/send} waits for the agent to finish.
     *
     * <p>The a2a SDK reads these from MicroProfile Config under CDI (defaults
     * 30s and 5s). Built by hand they are never injected: the constructor
     * leaves them at 0, and {@code DefaultRequestHandler.create} sets 5s and 2s.
     * Either way a model turn of more than a few seconds is answered "working"
     * with no final result, and a caller that does not poll waits forever. The
     * fields have no setter, so they are set here directly.</p>
     */
    private static void setBlockingTimeouts(DefaultRequestHandler handler, int agentSeconds) {
        try {
            java.lang.reflect.Field agent =
                DefaultRequestHandler.class.getDeclaredField("agentCompletionTimeoutSeconds");
            java.lang.reflect.Field consumption =
                DefaultRequestHandler.class.getDeclaredField("consumptionCompletionTimeoutSeconds");
            agent.setAccessible(true);
            consumption.setAccessible(true);
            agent.setInt(handler, agentSeconds);
            // Draining the last events after the agent finishes is quick.
            consumption.setInt(handler, 30);
        } catch (ReflectiveOperationException | RuntimeException e) {
            throw new IllegalStateException(
                "cannot set the A2A handler's blocking timeouts on this a2a-java version", e);
        }
    }

    /**
     * The card A2A clients read before calling: who the agent is, where to
     * reach it, and, in a Weil extension, which wallet serves it and what a
     * caller must hold.
     */
    private AgentCard buildAgentCard() {
        Map<String, Object> weil = new LinkedHashMap<>();
        weil.put("wallet", identity.address());
        if (requiredService != null) {
            weil.put("requires", requiredService);
            weil.put("purpose", "Execution or Management");
        }
        weil.put("headers", List.of(
            "X-Wallet-Address", "X-Signature", "X-Message", "X-Timestamp", "X-Body-SHA256"));
        weil.put("body_binding", "sha256 of params.message, keys sorted, compact, ASCII-escaped");

        String description = agent.description() == null || agent.description().isBlank()
            ? agent.name()
            : agent.description();
        return new AgentCard.Builder()
            .name(agent.name())
            .description(description)
            .url(publicUrl)
            .version("1.0.0")
            .protocolVersion(A2A_PROTOCOL_VERSION)
            .preferredTransport(TransportProtocol.JSONRPC.asString())
            .additionalInterfaces(List.of(
                new AgentInterface(TransportProtocol.JSONRPC.asString(), publicUrl)))
            .capabilities(new AgentCapabilities.Builder()
                .streaming(false)
                .pushNotifications(false)
                .stateTransitionHistory(false)
                .extensions(List.of(new AgentExtension.Builder()
                    .uri(WEIL_EXTENSION_URI)
                    .description("Every request must be signed by a Weil wallet; unsigned"
                        + " or unpermitted calls are refused before the agent runs.")
                    // Not marked required: a client that does not know the
                    // extension can still read the card, and is refused with a
                    // reason when it calls unsigned.
                    .required(false)
                    .params(weil)
                    .build()))
                .build())
            .defaultInputModes(List.of("text"))
            .defaultOutputModes(List.of("text"))
            .skills(List.of(new AgentSkill.Builder()
                .id(agent.name())
                .name(agent.name())
                .description(description)
                .tags(List.of(agent.name()))
                .build()))
            .build();
    }

    /** Unauthenticated, as A2A discovery is: the card holds nothing secret. */
    private void handleAgentCard(HttpExchange exchange) throws IOException {
        if (!"GET".equalsIgnoreCase(exchange.getRequestMethod())) {
            respondText(exchange, 405, "method not allowed");
            return;
        }
        respondBytes(exchange, 200, A2A_JSON.writeValueAsBytes(agentCard));
    }

    /** {@code POST /}: one A2A JSON-RPC call, after the Weil checks pass. */
    private void handleA2A(HttpExchange exchange) throws IOException {
        // "/" is HttpServer's catch-all context, so anything unrouted lands here.
        if (!"/".equals(exchange.getRequestURI().getPath())) {
            respondText(exchange, 404, "not found");
            return;
        }
        if (!"POST".equalsIgnoreCase(exchange.getRequestMethod())) {
            respondText(exchange, 405, "method not allowed");
            return;
        }

        long started = System.currentTimeMillis();
        String rawBody = read(exchange.getRequestBody());
        JsonNode envelope;
        try {
            envelope = A2A_JSON.readTree(rawBody);
        } catch (Exception e) {
            WeilA2A.trace(agent.name() + ": unreadable JSON-RPC body refused");
            respondRpc(exchange, new JSONRPCErrorResponse(null, new JSONParseError()));
            return;
        }
        String method = envelope.path("method").asText("?");
        WeilA2A.trace(agent.name() + ": " + method + " received from "
            + headersOf(exchange).getOrDefault("x-wallet-address", "unsigned caller"));

        Caller caller = authenticate(
            exchange, headersOf(exchange), WeilAuth.a2aSignedPayload(envelope));
        if (caller == null) {
            WeilA2A.trace(agent.name() + ": " + method + " refused after "
                + (System.currentTimeMillis() - started) + "ms");
            return;
        }
        WeilA2A.trace(agent.name() + ": " + method + " from " + caller + " passed checks after "
            + (System.currentTimeMillis() - started) + "ms; running");
        Object reply = dispatch(envelope, caller);
        respondRpc(exchange, reply);
        WeilA2A.trace(agent.name() + ": " + method + " answered (" + outcomeOf(reply) + ") in "
            + (System.currentTimeMillis() - started) + "ms");
    }

    /** A reply in a few words, for the trace: the task's state, or the error. */
    private static String outcomeOf(Object reply) {
        if (reply instanceof JSONRPCErrorResponse error) {
            return "error: " + error.getError().getMessage();
        }
        if (reply instanceof SendMessageResponse response) {
            if (response.getError() != null) {
                return "error: " + response.getError().getMessage();
            }
            if (response.getResult() instanceof Task task) {
                return "task " + task.getStatus().state();
            }
            return response.getResult() == null ? "empty" : "message";
        }
        return reply == null ? "empty" : reply.getClass().getSimpleName();
    }

    private Object dispatch(JsonNode envelope, Caller caller) {
        Object id = rpcId(envelope);
        String method = envelope.path("method").asText("");
        ServerCallContext context = callContext(caller);
        try {
            switch (method) {
                case SendMessageRequest.METHOD -> {
                    SendMessageRequest request = withCaller(
                        A2A_JSON.treeToValue(envelope, SendMessageRequest.class), caller);
                    Message message = request.getParams() == null
                        ? null
                        : request.getParams().message();
                    String contextId = message == null ? null : message.getContextId();
                    if (!claim(contextId, caller)) {
                        return new JSONRPCErrorResponse(id, new InvalidParamsError(
                            "context " + contextId + " belongs to another wallet"));
                    }
                    SendMessageResponse response = rpc.onMessageSend(request, context);
                    // A context the server opened is the caller's from here on.
                    claim(contextOf(response.getResult()), caller);
                    return response;
                }
                case GetTaskRequest.METHOD -> {
                    GetTaskRequest request = A2A_JSON.treeToValue(envelope, GetTaskRequest.class);
                    if (!ownsTask(request.getParams().id(), caller)) {
                        return new JSONRPCErrorResponse(id, new TaskNotFoundError());
                    }
                    return rpc.onGetTask(request, context);
                }
                case CancelTaskRequest.METHOD -> {
                    CancelTaskRequest request =
                        A2A_JSON.treeToValue(envelope, CancelTaskRequest.class);
                    if (!ownsTask(request.getParams().id(), caller)) {
                        return new JSONRPCErrorResponse(id, new TaskNotFoundError());
                    }
                    return rpc.onCancelTask(request, context);
                }
                case SendStreamingMessageRequest.METHOD, TaskResubscriptionRequest.METHOD -> {
                    return new JSONRPCErrorResponse(id, new UnsupportedOperationError(
                        null, "streaming is not offered; use message/send", null));
                }
                default -> {
                    return new JSONRPCErrorResponse(id, new MethodNotFoundError());
                }
            }
        } catch (com.fasterxml.jackson.core.JsonProcessingException | IllegalArgumentException e) {
            return new JSONRPCErrorResponse(id, new InvalidRequestError(e.getMessage()));
        } catch (Throwable t) {
            return new JSONRPCErrorResponse(id, new InternalError(String.valueOf(t.getMessage())));
        }
    }

    /**
     * The request with the verified caller stamped into its metadata, which
     * ADK's executor hands the run as {@code a2a_metadata}. Set after the
     * signature check, over any value the client sent, so a tool gate reads
     * who was verified rather than who the body claims to be.
     */
    private static SendMessageRequest withCaller(SendMessageRequest request, Caller caller) {
        MessageSendParams params = request.getParams();
        if (params == null) {
            return request;
        }
        Map<String, Object> metadata = new HashMap<>();
        if (params.metadata() != null) {
            metadata.putAll(params.metadata());
        }
        metadata.put(CALLER_METADATA_KEY, caller.canonicalAddress());
        return new SendMessageRequest(
            request.getJsonrpc(), request.getId(), request.getMethod(),
            new MessageSendParams(params.message(), params.configuration(), metadata));
    }

    /**
     * The wallet this server verified as the caller of the run behind
     * {@code ctx}, or null when the run was not started by a served request.
     */
    public static String callerWallet(InvocationContext ctx) {
        if (ctx == null || ctx.runConfig() == null) {
            return null;
        }
        Object metadata = ctx.runConfig().customMetadata().get(A2A_METADATA);
        if (metadata instanceof Map<?, ?> map
            && map.get(CALLER_METADATA_KEY) instanceof String wallet
            && !wallet.isBlank()) {
            return wallet;
        }
        return null;
    }

    /**
     * Bind {@code contextId} to the caller's wallet, or confirm it already is.
     *
     * @return false when another wallet started that conversation
     */
    private boolean claim(String contextId, Caller caller) {
        if (contextId == null || contextId.isBlank()) {
            return true;
        }
        String owner = contextOwners.putIfAbsent(contextId, caller.canonicalAddress());
        return owner == null || owner.equals(caller.canonicalAddress());
    }

    /** Whether the caller may see the task: unknown tasks pass, for the handler to report. */
    private boolean ownsTask(String taskId, Caller caller) {
        Task task = taskId == null ? null : taskStore.get(taskId);
        if (task == null) {
            return true;
        }
        String owner = contextOwners.get(task.getContextId());
        return owner == null || owner.equals(caller.canonicalAddress());
    }

    private static String contextOf(EventKind result) {
        if (result instanceof Task task) {
            return task.getContextId();
        }
        if (result instanceof Message message) {
            return message.getContextId();
        }
        return null;
    }

    /**
     * The A2A call context, carrying the verified caller: as the user, by its
     * canonical address, and whole under {@link #CALLER_STATE_KEY}.
     */
    private static ServerCallContext callContext(Caller caller) {
        Map<String, Object> state = new HashMap<>();
        state.put(CALLER_STATE_KEY, caller);
        return new ServerCallContext(new WalletUser(caller), state, Set.of());
    }

    private static Object rpcId(JsonNode envelope) {
        JsonNode id = envelope.get("id");
        if (id == null || id.isNull()) {
            return null;
        }
        return id.isNumber() ? id.numberValue() : id.asText();
    }

    /** A verified Weil wallet, as the a2a SDK's notion of a user. */
    private static final class WalletUser implements User {
        private final Caller caller;

        WalletUser(Caller caller) {
            this.caller = caller;
        }

        @Override
        public boolean isAuthenticated() {
            return true;
        }

        @Override
        public String getUsername() {
            return caller.canonicalAddress();
        }
    }

    /**
     * The Weil checks every request passes before the agent runs: the
     * signature and its binding to {@code signedBody}, the caller's permission
     * on the applet, then the optional authorizer.
     *
     * @return the verified caller, or null once a refusal has been sent
     */
    private Caller authenticate(HttpExchange exchange, Map<String, String> headers, Object signedBody)
        throws IOException {

        // 1. the signature, and the body it was signed over
        boolean verified = WeilAuth.verifyHeaders(
            headers.get("x-wallet-address"),
            headers.get("x-signature"),
            headers.get("x-message"),
            headers.get("x-timestamp"));
        if (!verified || !WeilAuth.verifyBody(canonicalHeaders(headers), signedBody)) {
            String wallet = headers.get("x-wallet-address");
            String message = notAuthorised(wallet,
                "its signature could not be verified (wallet address verification failed)");
            Map<String, Object> why = verifyFailureContext(headers, signedBody);
            why.put("wallet", wallet);
            why.put("message", message);
            auditRefusal("a2a.verify.failed", "rejected", 401, "verify", why);
            respondText(exchange, 401, message);
            return null;
        }

        String shortAddr = headers.get("x-wallet-address");
        Caller caller = new Caller(
            shortAddr,
            WeilAuth.canonicalAddress(shortAddr, headers.get("x-message")));

        // 2. on-chain authorization, on the canonical address the applet keys on
        if (requiredService != null) {
            String grantedBy;
            try {
                // Execution or Management, as Python's A2A gate asks: an admin
                // key is not made to enrol twice to call its own agents.
                grantedBy = PermissionGate.grantedBy(
                    caller.canonicalAddress(), requiredService, sentinelHost, true);
            } catch (Exception e) {
                // Could not ask, as opposed to being told no: say so distinctly,
                // or a wrong service name looks exactly like a denial.
                Map<String, Object> why = new LinkedHashMap<>();
                why.put("wallet", caller.canonicalAddress());
                why.put("reason", "permission check could not be completed");
                why.put("error", String.valueOf(e.getMessage()));
                why.put("error_type", e.getClass().getSimpleName());
                auditRefusal("a2a.permission.error", "error", 503, "permission", why);
                respondText(exchange, 503,
                    "permission check failed for " + requiredService + ": " + e.getMessage());
                return null;
            }
            publishVerdict(caller.canonicalAddress(), grantedBy);
            if (grantedBy == null) {
                String reason = "it holds neither Execution nor Management permission on "
                    + requiredService;
                String message = notAuthorised(caller.canonicalAddress(), reason);
                Map<String, Object> why = new LinkedHashMap<>();
                why.put("wallet", caller.canonicalAddress());
                why.put("reason", reason);
                why.put("message", message);
                auditRefusal("a2a.permission.denied", "denied", 403, "permission", why);
                respondText(exchange, 403, message);
                return null;
            }
        }
        if (authorizer != null) {
            String reason = authorizer.refuse(caller);
            if (reason != null) {
                String message = notAuthorised(caller.canonicalAddress(), reason);
                Map<String, Object> why = new LinkedHashMap<>();
                why.put("wallet", caller.canonicalAddress());
                why.put("reason", reason);
                why.put("message", message);
                auditRefusal("a2a.authorizer.denied", "denied", 403, "authorizer", why);
                respondText(exchange, 403, message);
                return null;
            }
        }
        return caller;
    }

    /**
     * Publish the permission check's verdict to WeilStream, as the Python SDK
     * does for every {@code @secured} check, signed by this agent. Sent
     * fire-and-forget: the caller waits for the platform to accept it, not
     * for the chain to execute it.
     */
    private void publishVerdict(String checkedWallet, String grantedBy) {
        if (!publishPermissionChecks) {
            return;
        }
        try {
            auditQueue.submit(identity,
                PermissionGate.agentVerdictCall(
                    checkedWallet, agent.name(), requiredService, grantedBy), null);
        } catch (Exception ignored) {
            // recording the verdict must not change it
        }
    }

    /**
     * The refusal a caller reads, and through it the calling agent's model:
     * which wallet was turned away, from what, and why, in one sentence it can
     * pass on to its user as it stands.
     */
    private String notAuthorised(String wallet, String reason) {
        String who = wallet == null || wallet.isBlank() ? "unsigned request" : "wallet " + wallet;
        return who + " is not authorised to perform operations on " + agent.name()
            + ": " + reason;
    }

    // ------------------------------------------------------------------

    private static String read(InputStream in) throws IOException {
        return new String(in.readAllBytes(), StandardCharsets.UTF_8);
    }

    private static Map<String, String> headersOf(HttpExchange exchange) {
        Map<String, String> out = new LinkedHashMap<>();
        exchange.getRequestHeaders().forEach((k, v) -> {
            if (!v.isEmpty()) {
                out.put(k.toLowerCase(), v.get(0));
            }
        });
        return out;
    }

    /** WeilAuth reads canonical header names; the wire gives us lowercase. */
    private static Map<String, String> canonicalHeaders(Map<String, String> lower) {
        Map<String, String> out = new LinkedHashMap<>();
        put(out, lower, "X-Wallet-Address");
        put(out, lower, "X-Signature");
        put(out, lower, "X-Message");
        put(out, lower, "X-Timestamp");
        put(out, lower, "X-Body-SHA256");
        put(out, lower, "X-Nonce");
        return out;
    }

    private static void put(Map<String, String> out, Map<String, String> lower, String name) {
        String v = lower.get(name.toLowerCase());
        if (v != null) {
            out.put(name, v);
        }
    }

    private static void respondRpc(HttpExchange exchange, Object response) throws IOException {
        // JSON-RPC reports its own errors in the body; the HTTP call succeeded.
        respondBytes(exchange, 200, A2A_JSON.writeValueAsBytes(response));
    }

    private static void respond(HttpExchange exchange, int status, Object body) throws IOException {
        respondBytes(exchange, status, JSON.writeValueAsBytes(body));
    }

    private static void respondBytes(HttpExchange exchange, int status, byte[] bytes)
        throws IOException {
        exchange.getResponseHeaders().add("Content-Type", "application/json");
        exchange.sendResponseHeaders(status, bytes.length);
        try (OutputStream out = exchange.getResponseBody()) {
            out.write(bytes);
        }
    }

    private static void respondText(HttpExchange exchange, int status, String message)
        throws IOException {
        byte[] bytes = message.getBytes(StandardCharsets.UTF_8);
        exchange.getResponseHeaders().add("Content-Type", "text/plain; charset=utf-8");
        exchange.sendResponseHeaders(status, bytes.length);
        try (OutputStream out = exchange.getResponseBody()) {
            out.write(bytes);
        }
    }

    // ------------------------------------------------------------------

    public static final class Builder {
        private final BaseAgent agent;
        private final App app;
        private final WeilIdentity identity;
        private String appName;
        private int port = portFromEnv(8001);
        private String bindAddress;
        private String publicUrl;
        private int agentTimeoutSeconds = (int) WeilA2A.TIMEOUT_SECONDS;
        private String requiredService;
        private String sentinelHost;
        private Authorizer authorizer;
        private final List<Plugin> plugins = new ArrayList<>();
        private boolean auditRefusals = true;
        private boolean publishPermissionChecks = true;

        private Builder(BaseAgent agent, App app, WeilIdentity identity) {
            this.agent = agent;
            this.app = app;
            this.identity = identity;
        }

        public Builder appName(String appName) {
            this.appName = appName;
            return this;
        }

        /** The port to serve on. Defaults to {@code PORT} from the environment, else 8001. */
        public Builder port(int port) {
            this.port = port;
            return this;
        }

        /**
         * The address to listen on. Defaults to {@code WEIL_A2A_BIND}, else
         * {@code 127.0.0.1}, so only this machine can call; a container sets
         * {@code 0.0.0.0} to take calls from other hosts.
         */
        public Builder bindAddress(String bindAddress) {
            this.bindAddress = bindAddress == null || bindAddress.isBlank() ? null : bindAddress.trim();
            return this;
        }

        /**
         * The URL callers reach the agent at, as published in its A2A card.
         * Clients send their calls to this URL, not the one they fetched the
         * card from. Defaults to {@code WEIL_A2A_PUBLIC_URL}, else
         * {@code http://<bind address>:<port>/} ({@code 127.0.0.1} when
         * listening on all interfaces); set it when the agent sits behind a
         * proxy, in a container, or on another host name.
         */
        public Builder publicUrl(String publicUrl) {
            this.publicUrl = publicUrl;
            return this;
        }

        /**
         * How long one A2A call may run the agent before the caller is answered
         * with the task still working. Defaults to {@code WEIL_A2A_TIMEOUT_SECONDS}
         * (300), the caller's own limit.
         */
        public Builder agentTimeoutSeconds(int seconds) {
            this.agentTimeoutSeconds = seconds;
            return this;
        }

        /**
         * Require callers to hold Execution or Management purpose on this
         * service, as Python's A2A gate does. Null or
         * blank leaves the agent open to any verified signature, so a value
         * read from an unset environment variable turns the gate off.
         */
        public Builder requirePermission(String svcName) {
            this.requiredService = svcName == null || svcName.isBlank() ? null : svcName.trim();
            return this;
        }

        public Builder sentinelHost(String sentinelHost) {
            this.sentinelHost = sentinelHost;
            return this;
        }

        /** Extra check on a verified caller, e.g. an allowlist of wallets. */
        public Builder authorize(Authorizer authorizer) {
            this.authorizer = authorizer;
            return this;
        }

        /**
         * Stop recording refused requests on the auditor applet.
         *
         * <p>On by default. Turning it off makes an unauthorized call leave no
         * trace, so the reason to do it is cost, not noise.</p>
         */
        public Builder auditRefusals(boolean auditRefusals) {
            this.auditRefusals = auditRefusals;
            return this;
        }

        /**
         * Stop publishing each permission check's verdict to the
         * {@code security} WeilStream topic. On by default, as in the
         * Python SDK.
         */
        public Builder publishPermissionChecks(boolean publish) {
            this.publishPermissionChecks = publish;
            return this;
        }

        /** Plugins for the served agent's own runner, typically a WeilPlugin. */
        public Builder plugins(List<Plugin> plugins) {
            this.plugins.addAll(plugins);
            return this;
        }

        public WeilAgentServer build() {
            return new WeilAgentServer(this);
        }
    }
    // ── Refusing is an auditable event ───────────────────────────────────────

    /**
     * Record a refused request on the auditor applet.
     *
     * <p>The plugin's hooks only fire once the agent runs, so without this a
     * rejected call leaves no trace at all — and an unauthorized wallet
     * attempting a call is exactly what a trail is for. Mirrors the Python
     * SDK's {@code a2a.verify.failed} / {@code a2a.permission.denied} /
     * {@code a2a.permission.error} records.</p>
     *
     * <p>Queued, never blocking: the caller gets its 401 or 403 immediately,
     * and a chain write must not sit in the path of a rejection. Failures to
     * audit are swallowed — refusing the request already succeeded, and
     * throwing here would turn a clean denial into a 500.</p>
     */
    private void auditRefusal(
        String event, String outcome, int status, String stage, Map<String, Object> context) {

        if (!auditRefusals) {
            return;
        }
        try {
            Map<String, Object> record = new LinkedHashMap<>();
            record.put("v", 1);
            record.put("event", event);
            record.put("ts", DateTimeFormatter.ISO_INSTANT.format(Instant.now()));
            record.put("outcome", outcome);
            record.put("status", status);
            record.put("stage", stage);
            record.put("agent", agent.name());
            record.put("served_by", identity.address());
            if (requiredService != null) {
                record.put("service", requiredService);
            }
            context.forEach((k, value) -> {
                if (value != null) {
                    record.put(k, value);
                }
            });

            String argsJson = identity.client().auditArgs(JSON.writeValueAsString(record));
            auditQueue.submit(identity, Call.to(Applet.auditor(), "audit")
                .argsJson(argsJson)
                .hideArgs(false)
                .fireAndForget(true)
                .build(), null);
        } catch (Exception ignored) {
            // Auditing a refusal must not change the refusal.
        }
    }

    /**
     * Why verification failed, so a 401 is actionable without a reproduction.
     *
     * <p>Separates the failures that look identical from outside: headers never
     * sent, a stale or replayed timestamp, and a body that is not the one that
     * was signed. Anything else is a signature mismatch.</p>
     */
    private static Map<String, Object> verifyFailureContext(
        Map<String, String> headers, Object signedBody) {

        Map<String, Object> why = new LinkedHashMap<>();
        List<String> missing = new ArrayList<>();
        for (String name : REQUIRED_AUTH_HEADERS) {
            String value = headers.get(name);
            if (value == null || value.isBlank()) {
                missing.add(name);
            }
        }
        if (!missing.isEmpty()) {
            why.put("reason", "missing_auth_headers");
            why.put("missing_headers", missing);
            return why;
        }

        String timestamp = headers.get("x-timestamp");
        long skew;
        try {
            skew = (System.currentTimeMillis() / 1000L) - Long.parseLong(timestamp.trim());
        } catch (Exception e) {
            why.put("reason", "malformed_timestamp");
            why.put("timestamp", timestamp);
            return why;
        }
        if (Math.abs(skew) > WeilAuth.MAX_TIMESTAMP_AGE_SECONDS) {
            why.put("reason", "stale_timestamp");
            why.put("timestamp_skew_s", skew);
            why.put("max_age_s", WeilAuth.MAX_TIMESTAMP_AGE_SECONDS);
            return why;
        }

        JsonNode signed;
        try {
            signed = JSON.readTree(headers.get("x-message"));
        } catch (Exception e) {
            why.put("reason", "malformed_signed_message");
            return why;
        }
        if (signed == null || !signed.isObject()) {
            why.put("reason", "malformed_signed_message");
            return why;
        }

        String signedHash = signed.path("body_sha256").asText(null);
        if (signedHash == null || signedHash.isBlank()) {
            why.put("reason", "unbound_body");
            why.put("detail", "signed message omits body_sha256");
            return why;
        }
        String actualHash;
        try {
            actualHash = WeilAuth.bodySha256(signedBody);
        } catch (Exception e) {
            actualHash = null;
        }
        if (actualHash != null && !signedHash.equals(actualHash)) {
            why.put("reason", "body_hash_mismatch");
            why.put("signed_body_sha256", shortHash(signedHash));
            why.put("actual_body_sha256", shortHash(actualHash));
            return why;
        }

        why.put("reason", "signature_mismatch");
        return why;
    }

    /** Enough of a digest to compare two by eye, without reproducing the body. */
    private static String shortHash(String hash) {
        return hash == null || hash.length() <= 12 ? hash : hash.substring(0, 12) + "...";
    }
}
