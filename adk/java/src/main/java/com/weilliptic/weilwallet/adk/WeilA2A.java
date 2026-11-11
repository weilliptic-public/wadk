package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.JsonNode;
import com.google.adk.a2a.agent.RemoteA2AAgent;
import com.google.adk.tools.AgentTool;
import com.google.adk.tools.BaseTool;
import com.google.adk.tools.ToolContext;
import com.google.genai.types.FunctionDeclaration;
import com.weilliptic.weilwallet.TlsMode;
import com.weilliptic.weilwallet.WeilAuth;

import io.a2a.client.Client;
import io.a2a.client.config.ClientConfig;
import io.a2a.client.http.A2ACardResolver;
import io.a2a.client.http.A2AHttpClient;
import io.a2a.client.http.A2AHttpResponse;
import io.a2a.client.http.JdkA2AHttpClient;
import io.a2a.client.transport.jsonrpc.JSONRPCTransport;
import io.a2a.client.transport.jsonrpc.JSONRPCTransportConfigBuilder;
import io.a2a.client.transport.spi.interceptors.ClientCallContext;
import io.a2a.client.transport.spi.interceptors.ClientCallInterceptor;
import io.a2a.client.transport.spi.interceptors.PayloadAndHeaders;
import io.a2a.spec.AgentCapabilities;
import io.a2a.spec.AgentCard;
import io.a2a.spec.TransportProtocol;
import io.a2a.util.Utils;

import io.reactivex.rxjava3.core.Single;
import io.reactivex.rxjava3.schedulers.Schedulers;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/**
 * Calls other agents over A2A, every request signed by this agent's wallet.
 *
 * <p>The calling side of {@link WeilAgentServer}. It builds ADK's own
 * {@link RemoteA2AAgent} on the a2a-java client, adding one thing: an
 * interceptor that signs each JSON-RPC call with the caller's wallet, binding
 * the signature to the A2A message the way the Python SDK does. So the remote
 * agent can be any A2A server that checks Weil signatures, in Java, Python or
 * another language.</p>
 *
 * <pre>{@code
 * // as a sub-agent the model can transfer to
 * RemoteA2AAgent flights = WeilA2A.remoteAgent("flight_agent", "http://127.0.0.1:8001", identity);
 *
 * // or as a tool, so a coordinator can call several and merge the answers
 * BaseTool flights = WeilA2A.remoteTool("flight_agent", "http://127.0.0.1:8001", identity);
 * }</pre>
 *
 * <p>A refused call reaches the calling model as the remote server's own
 * words: a tool returns {@code {"error": "HTTP 403: wallet ... is not
 * authorised ..."}} rather than failing the turn, so the model can explain
 * the refusal to its user.</p>
 *
 * <p>The agent card is fetched when the remote agent is built. When the remote
 * server is not up yet, a stand-in card naming only its URL is used, so the
 * calling agent still starts; its calls fail until the server is up.</p>
 */
public final class WeilA2A {

    /**
     * How long a remote tool call may take before it is answered with an
     * error, from {@code WEIL_A2A_TIMEOUT_SECONDS}. The remote agent runs its
     * own model, and tools, so this is minutes rather than seconds.
     */
    static final long TIMEOUT_SECONDS = longEnv("WEIL_A2A_TIMEOUT_SECONDS", 300);

    /** {@code WEIL_A2A_DEBUG=0} silences the one-line trace of each remote call. */
    static final boolean DEBUG = !"0".equals(System.getenv("WEIL_A2A_DEBUG"));

    private WeilA2A() {}

    /** One trace line on stderr, stamped, so a stalled call shows how far it got. */
    static void trace(String message) {
        if (DEBUG) {
            System.err.println("[weil-a2a " + java.time.LocalTime.now().format(java.time.format.DateTimeFormatter.ofPattern("HH:mm:ss")) + "] " + message);
        }
    }

    private static long longEnv(String name, long fallback) {
        try {
            String value = System.getenv(name);
            return value == null || value.isBlank() ? fallback : Long.parseLong(value.trim());
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    /**
     * A remote A2A agent, its calls signed by {@code identity}.
     *
     * @param name    what the calling model knows the agent as
     * @param url     the remote agent's base URL, or its agent card's URL
     * @param identity the wallet that signs every call
     */
    public static RemoteA2AAgent remoteAgent(String name, String url, WeilIdentity identity) {
        return remoteAgent(name, url, identity, null);
    }

    /**
     * As {@link #remoteAgent(String, String, WeilIdentity)}, with the
     * description the calling model reads to decide when to use the agent.
     * Defaults to the description on the remote agent's card.
     */
    public static RemoteA2AAgent remoteAgent(
        String name, String url, WeilIdentity identity, String description) {

        Objects.requireNonNull(name, "name");
        Objects.requireNonNull(url, "url");
        Objects.requireNonNull(identity, "identity");

        AgentCard card = cardOrStandIn(name, url, description);
        return RemoteA2AAgent.builder()
            .name(name)
            .description(description != null ? description : card.description())
            .agentCard(card)
            .a2aClient(client(card, identity))
            .streaming(false)
            .build();
    }

    /**
     * A remote A2A agent held as a tool, so a coordinator can call several in
     * one turn and combine their answers rather than handing over control.
     * Those calls run at the same time, each on a thread of its own, whatever
     * the run's tool execution mode, so the turn waits for the slowest one.
     *
     * <p>A failed call returns {@code {"error": reason}} to the model instead
     * of failing the turn, and so does one that takes longer than
     * {@code WEIL_A2A_TIMEOUT_SECONDS} (default 300).</p>
     */
    public static BaseTool remoteTool(String name, String url, WeilIdentity identity) {
        return remoteTool(name, url, identity, null);
    }

    /** As {@link #remoteTool(String, String, WeilIdentity)}, with a description for the model. */
    public static BaseTool remoteTool(
        String name, String url, WeilIdentity identity, String description) {
        return new RemoteTool(AgentTool.create(remoteAgent(name, url, identity, description)));
    }

    /**
     * An a2a-java client for {@code card}, signing every call as
     * {@code identity}. For calling an A2A agent directly, outside an ADK
     * agent tree.
     */
    public static Client client(AgentCard card, WeilIdentity identity) {
        try {
            return Client.builder(card)
                .clientConfig(ClientConfig.builder()
                    .setStreaming(false)
                    .setAcceptedOutputModes(List.of("text"))
                    .build())
                .withTransport(JSONRPCTransport.class, new JSONRPCTransportConfigBuilder()
                    .httpClient(new ExplainingHttpClient(new JdkA2AHttpClient()))
                    .addInterceptor(signer(identity)))
                .build();
        } catch (Exception e) {
            throw new IllegalStateException(
                "could not build an A2A client for " + card.name() + ": " + e.getMessage(), e);
        }
    }

    /** The interceptor that adds Weil auth headers to each outgoing A2A call. */
    public static ClientCallInterceptor signer(WeilIdentity identity) {
        return new Signer(identity);
    }

    /**
     * Read the agent card from {@code url}: a base URL, to which the standard
     * card path is added, or the card's own URL.
     */
    public static AgentCard fetchCard(String url) {
        String base = cardBase(url);
        try {
            return new A2ACardResolver(new JdkA2AHttpClient(), base).getAgentCard();
        } catch (Exception e) {
            throw new IllegalStateException(
                "could not read the A2A agent card at " + base + ": " + e.getMessage(), e);
        }
    }

    /** {@code url} without a trailing agent card path: the agent's base URL. */
    private static String cardBase(String url) {
        String base = url.trim();
        for (String path : List.of(
            WeilAgentServer.AGENT_CARD_PATH, WeilAgentServer.LEGACY_AGENT_CARD_PATH)) {
            if (base.endsWith(path)) {
                base = base.substring(0, base.length() - path.length());
            }
        }
        return base;
    }

    /**
     * The remote agent's card, or, when it cannot be read yet, a stand-in
     * naming only its URL. The agent that calls it can then start, and answer
     * a health check, before the remote server is up; calls go to that URL
     * and fail, as tool errors the model can report, until it is.
     */
    private static AgentCard cardOrStandIn(String name, String url, String description) {
        try {
            return fetchCard(url);
        } catch (IllegalStateException e) {
            String base = cardBase(url);
            System.err.println("[weil-a2a] " + e.getMessage()
                + "; calling " + base + " anyway, which fails until it is up");
            return new AgentCard.Builder()
                .name(name)
                .description(description != null ? description : "The A2A agent at " + base)
                .url(base)
                .version("unknown")
                .protocolVersion(WeilAgentServer.A2A_PROTOCOL_VERSION)
                .preferredTransport(TransportProtocol.JSONRPC.asString())
                .capabilities(new AgentCapabilities.Builder()
                    .streaming(false)
                    .pushNotifications(false)
                    .stateTransitionHistory(false)
                    .build())
                .defaultInputModes(List.of("text"))
                .defaultOutputModes(List.of("text"))
                .skills(List.of())
                .build();
        }
    }

    /**
     * The reason a remote call failed, as plainly as the failure says it: the
     * innermost message, which for a refusal is the remote server's own
     * explanation.
     */
    static String explain(Throwable error) {
        String reason = null;
        for (Throwable t = error; t != null; t = t.getCause()) {
            if (t.getMessage() != null && !t.getMessage().isBlank()) {
                reason = t.getMessage();
            }
            if (t.getCause() == t) {
                break;
            }
        }
        return reason != null ? reason : error.getClass().getSimpleName();
    }

    /**
     * An agent tool whose failures come back as a result. A remote agent that
     * refuses or cannot be reached otherwise fails the whole turn, and the
     * model never learns why.
     */
    private static final class RemoteTool extends BaseTool {
        private final AgentTool delegate;

        RemoteTool(AgentTool delegate) {
            super(delegate.name(), delegate.description());
            this.delegate = delegate;
        }

        @Override
        public Optional<FunctionDeclaration> declaration() {
            return delegate.declaration();
        }

        @Override
        public Single<Map<String, Object>> runAsync(Map<String, Object> args, ToolContext toolContext) {
            long started = System.currentTimeMillis();
            return Single.defer(() -> {
                    trace(name() + ": tool call started");
                    return delegate.runAsync(args, toolContext);
                })
                // ADK's remote agent waits for its HTTP reply on the thread
                // that starts it, and by default ADK starts the tools of one
                // turn on the same thread, so they would run one after the
                // other. On a thread of its own, each remote call overlaps the
                // others the model asked for in that turn.
                .subscribeOn(Schedulers.io())
                .timeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
                .doOnSuccess(result -> trace(name() + ": tool call returned in "
                    + (System.currentTimeMillis() - started) + "ms"))
                .onErrorReturn(error -> {
                    String reason = error instanceof java.util.concurrent.TimeoutException
                        ? "no answer within " + TIMEOUT_SECONDS + "s"
                        : explain(error);
                    trace(name() + ": tool call failed after "
                        + (System.currentTimeMillis() - started) + "ms: " + reason);
                    return Map.of("error", name() + " could not answer: " + reason);
                });
        }
    }

    /**
     * Keeps the body of a failed HTTP response. The a2a JSON-RPC transport
     * reports only the status of a non-2xx reply, which drops exactly what a
     * Weil server puts there: why this wallet was refused.
     */
    private static final class ExplainingHttpClient implements A2AHttpClient {
        private final A2AHttpClient delegate;

        ExplainingHttpClient(A2AHttpClient delegate) {
            this.delegate = delegate;
        }

        @Override
        public GetBuilder createGet() {
            return delegate.createGet();
        }

        @Override
        public DeleteBuilder createDelete() {
            return delegate.createDelete();
        }

        @Override
        public PostBuilder createPost() {
            return new ExplainingPost(delegate.createPost());
        }
    }

    private static final class ExplainingPost implements A2AHttpClient.PostBuilder {
        /** Shared, as a2a's own client shares one; honours the SDK's TLS mode. */
        private static final HttpClient HTTP = TlsMode.httpClient();

        private final A2AHttpClient.PostBuilder delegate;
        private final Map<String, String> headers = new LinkedHashMap<>();
        private String url;
        private String body = "";

        ExplainingPost(A2AHttpClient.PostBuilder delegate) {
            this.delegate = delegate;
        }

        @Override
        public A2AHttpClient.PostBuilder url(String url) {
            this.url = url;
            delegate.url(url);
            return this;
        }

        @Override
        public A2AHttpClient.PostBuilder addHeaders(Map<String, String> headers) {
            this.headers.putAll(headers);
            delegate.addHeaders(headers);
            return this;
        }

        @Override
        public A2AHttpClient.PostBuilder addHeader(String name, String value) {
            headers.put(name, value);
            delegate.addHeader(name, value);
            return this;
        }

        @Override
        public A2AHttpClient.PostBuilder body(String body) {
            this.body = body;
            delegate.body(body);
            return this;
        }

        @Override
        public A2AHttpResponse post() throws IOException, InterruptedException {
            long started = System.currentTimeMillis();
            trace("POST " + url + " sent");
            A2AHttpResponse response;
            try {
                // Sent here, the same request a2a's JDK client would send,
                // rather than through it: that client turns a 401 or 403 into
                // a fixed sentence and drops the body, which is where a Weil
                // server says which wallet was refused and why.
                response = send();
            } catch (IOException | InterruptedException | RuntimeException e) {
                trace("POST " + url + " failed after " + (System.currentTimeMillis() - started)
                    + "ms: " + explain(e));
                throw e;
            }
            trace("POST " + url + " -> HTTP " + response.status() + " in "
                + (System.currentTimeMillis() - started) + "ms");
            if (!response.success()) {
                String body = response.body();
                throw new IOException("HTTP " + response.status()
                    + (body == null || body.isBlank() ? "" : ": " + body.trim()));
            }
            return response;
        }

        private A2AHttpResponse send() throws IOException, InterruptedException {
            HttpRequest.Builder request = HttpRequest.newBuilder(URI.create(url))
                .POST(HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8));
            headers.forEach(request::header);
            HttpResponse<String> response =
                HTTP.send(request.build(), HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8));
            return new A2AHttpResponse() {
                @Override
                public int status() {
                    return response.statusCode();
                }

                @Override
                public boolean success() {
                    return response.statusCode() >= 200 && response.statusCode() < 300;
                }

                @Override
                public String body() {
                    return response.body();
                }
            };
        }

        @Override
        public CompletableFuture<Void> postAsyncSSE(
            Consumer<String> messageConsumer,
            Consumer<Throwable> errorConsumer,
            Runnable completeRunnable) throws IOException, InterruptedException {
            return delegate.postAsyncSSE(messageConsumer, errorConsumer, completeRunnable);
        }
    }

    /**
     * Signs each call over the part a Weil server checks: the A2A message in
     * {@code params.message}, or the params themselves for task calls. It
     * hashes the payload as the transport will send it, so what the server
     * re-hashes from the wire is what was signed.
     */
    private static final class Signer extends ClientCallInterceptor {
        private final WeilIdentity identity;

        Signer(WeilIdentity identity) {
            this.identity = identity;
        }

        @Override
        public PayloadAndHeaders intercept(
            String methodName,
            Object payload,
            Map<String, String> headers,
            AgentCard agentCard,
            ClientCallContext clientCallContext) {

            JsonNode wire;
            try {
                wire = Utils.OBJECT_MAPPER.readTree(Utils.OBJECT_MAPPER.writeValueAsString(payload));
            } catch (Exception e) {
                throw new IllegalStateException("could not encode the A2A " + methodName
                    + " call to sign it: " + e.getMessage(), e);
            }
            Map<String, String> signed = new HashMap<>();
            if (headers != null) {
                signed.putAll(headers);
            }
            long started = System.currentTimeMillis();
            try {
                signed.putAll(identity.client().buildAuthHeaders(WeilAuth.a2aSignedPayload(wire)));
            } catch (RuntimeException e) {
                trace(methodName + " to " + agentCard.name() + ": signing failed: " + explain(e));
                throw e;
            }
            trace(methodName + " to " + agentCard.name() + ": signed in "
                + (System.currentTimeMillis() - started) + "ms");
            return new PayloadAndHeaders(payload, signed);
        }
    }
}
