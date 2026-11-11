package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.apps.App;
import com.google.adk.plugins.BasePlugin;
import com.google.adk.plugins.Plugin;
import com.google.adk.runner.InMemoryRunner;
import com.google.adk.runner.Runner;
import com.google.adk.sessions.Session;
import com.google.adk.tools.AgentTool;
import com.google.adk.tools.BaseTool;
import com.google.genai.types.Content;
import com.google.genai.types.Part;

import com.weilliptic.weilwallet.TraceAction;
import com.weilliptic.weilwallet.WeilClient;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.function.Supplier;

/**
 * Weil for an ADK agent in one line: the wallet, the audit trail, the tool
 * permissions, and serving or running the agent, all set up from the env file.
 *
 * <pre>{@code
 * public static class Tools {
 *     @WeilSecured("engg.weil")                 // only callers permitted there
 *     public static Map<String, Object> searchFlights(...) { ... }
 * }
 *
 * LlmAgent agent = LlmAgent.builder()...tools(FunctionTool.create(Tools.class, "searchFlights")).build();
 *
 * Weil.toSecureA2a(agent, 8001);               // callers checked on WEIL_SERVICE
 * Weil.toSecureA2a(agent, 8001, "engg.weil");  // or on the service given here
 * Weil.chat(agent, args, "a question");        // or ask it at the terminal (-i: type as you go)
 * Weil.web(agent, args);                       // or in ADK's dev UI, on PORT
 * }</pre>
 *
 * <p>Calling other Weil agents and remembering, with the same wallet:</p>
 *
 * <pre>{@code
 * .tools(Weil.withMemory(Weil.remoteTool("flight_agent", flightsUrl, "Searches flights.")))
 * }</pre>
 *
 * <p>What it reads from the environment:</p>
 * <ul>
 *   <li>{@code WEIL_AGENT_KEY} (or {@code WEIL_WALLET_FILE}) and
 *       {@code SENTINEL_HOST}: the agent's wallet, loaded once per process
 *       and shared by everything here;</li>
 *   <li>{@code WEIL_SERVICE}: for {@link #toSecureA2a} given no service, the
 *       subgroup a caller must hold Execution or Management in to reach the
 *       agent at all (HTTP 403 before the agent runs); unset, signatures are
 *       checked and nothing else.</li>
 * </ul>
 *
 * <p>Every run gets the same plugin: an on-chain audit record of each step,
 * a trace of the run (the prompt and every tool call) kept on
 * {@code weilchain::weil} and published to {@code publisher::<org>} on topic
 * {@code audits} when the run ends, as the Python SDK's tracer does, and a
 * gate on every tool method carrying {@link WeilSecured}, on the service the
 * annotation names. Its records are sent from a background queue, one at a
 * time in the order made, so the agent waits only for the gates; whatever is
 * still queued is sent before the process exits, when a chat ends or the
 * process is stopped. The wallet starts loading in the background as soon as
 * the plugin is made.</p>
 *
 * <p>For more than one wallet in a process, or a plugin of another shape,
 * use {@link WeilIdentity}, {@link WeilPlugin} and {@link WeilAgentServer}
 * directly; {@link #identity()} hands over the wallet this class uses.</p>
 */
public final class Weil {

    /** Where each run's finished trace is published. */
    public static final String AUDIT_TOPIC = TraceAction.TOPIC;

    private static WeilIdentity identity;
    private static final List<WeilPlugin> plugins = new ArrayList<>();
    private static boolean shutDown;
    private static boolean warming;

    /** Background attempts to load the wallet before anything needs it. */
    private static final int WARM_ATTEMPTS = 3;

    private Weil() {}

    /**
     * The agent's wallet, from WEIL_AGENT_KEY (with SENTINEL_HOST when set)
     * or WEIL_WALLET_FILE. The wallet itself is loaded the first time it is
     * used, not here: plugins, servers and the web UI can be set up, and
     * start, before it is reachable. Stopping the process flushes and closes it.
     */
    public static synchronized WeilIdentity identity() {
        if (identity == null) {
            identity = loadIdentity();
            Runtime.getRuntime().addShutdownHook(new Thread(Weil::shutdown, "weil-shutdown"));
        }
        return identity;
    }

    /** Use {@code wallet} instead of the env's; for tests, before anything else here. */
    static synchronized void useIdentity(WeilIdentity wallet) {
        identity = wallet;
    }

    /**
     * A plugin for the agent: audit trail, the run's trace, the
     * {@link WeilSecured} gates, then {@code extra} actions, such as a
     * {@link PermissionGate} for tools that cannot be annotated.
     *
     * <p>Audits and traces go on the plugin's ordered background queue, so
     * the agent never waits on them; only a gate's verdict is waited for. The
     * queue is not flushed after each run, only at exit, where every record
     * still queued is sent before the process ends.</p>
     */
    public static synchronized WeilPlugin plugin(WeilAction... extra) {
        WeilIdentity id = identity();
        warm(id);
        WeilPlugin plugin = WeilPlugin.builder(id)
            .name("weil-audit")
            .inBackground()
            .withActions(AuditAction.withContent())
            // One trace per run, published to "audits" when it ends. Best
            // effort: a trace that cannot be written never stops the agent.
            .withActions(TraceAction.actions())
            .withActions(PermissionGate.annotated(id, System.getenv("SENTINEL_HOST")))
            .withActions(extra)
            .onError((ev, action, err) ->
                System.out.println("  [weil] " + action + " on " + ev.hook() + ": " + err))
            .build();
        plugins.add(plugin);
        return plugin;
    }

    /**
     * Start loading the wallet on a background thread, so the first message
     * does not wait for it. Nothing waits for this: a load that fails, such
     * as on a placeholder key, is logged once after a few attempts and tried
     * again when something first needs the wallet.
     */
    private static void warm(WeilIdentity id) {
        if (warming || id.isLoaded()) {
            return;
        }
        warming = true;
        Thread loader = new Thread(() -> {
            RuntimeException last = null;
            for (int attempt = 1; attempt <= WARM_ATTEMPTS; attempt++) {
                try {
                    id.client();
                    return;
                } catch (RuntimeException e) {
                    last = e;
                }
                if (attempt < WARM_ATTEMPTS) {
                    try {
                        Thread.sleep(2_000L << (attempt - 1));
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
            System.out.println("  [weil] could not load the agent's wallet in the background ("
                + WARM_ATTEMPTS + " attempts): " + last.getMessage()
                + "; it is tried again when first needed");
        }, "weil-wallet");
        loader.setDaemon(true);
        loader.start();
    }

    /**
     * {@link #plugin} as the list a runner, or ADK's web UI, takes, with
     * {@link MemoryPrompt#plugin()}: an agent holding the memory tools is told
     * how to use them, with no instruction of its own to add.
     */
    public static List<BasePlugin> plugins(WeilAction... extra) {
        return List.of(plugin(extra), MemoryPrompt.plugin());
    }

    /** A runner for {@code agent} with the Weil plugin. */
    public static InMemoryRunner runner(BaseAgent agent, WeilAction... extra) {
        return new InMemoryRunner(agent, agent.name(), plugins(extra));
    }

    /**
     * A server for {@code agent} over A2A, ready but for its port: callers
     * sign with their wallet, need Execution or Management in WEIL_SERVICE
     * when it is set, and every run has the Weil plugin. Set other options
     * before {@code build().start()}.
     */
    public static WeilAgentServer.Builder secureA2aServer(BaseAgent agent, WeilAction... extra) {
        return secureA2aServer(agent, System.getenv("WEIL_SERVICE"), extra);
    }

    /**
     * As {@link #secureA2aServer(BaseAgent, WeilAction...)}, with callers
     * checked on {@code service}, such as {@code "engg.weil"}, instead of WEIL_SERVICE:
     * written in the code, as {@link WeilSecured} is on a tool. Null or blank
     * leaves the agent open to any verified signature.
     */
    public static WeilAgentServer.Builder secureA2aServer(
        BaseAgent agent, String service, WeilAction... extra) {
        return WeilAgentServer.builder(agent, identity())
            .requirePermission(service)
            .sentinelHost(System.getenv("SENTINEL_HOST"))
            .plugins(new ArrayList<Plugin>(plugins(extra)));
    }

    /** Serve {@code agent} over A2A on {@code port}, callers checked on WEIL_SERVICE. */
    public static void toSecureA2a(BaseAgent agent, int port, WeilAction... extra)
        throws IOException {
        secureA2aServer(agent, extra).port(port).build().start();
    }

    /** Serve {@code agent} over A2A on {@code port}, callers checked on {@code service}. */
    public static void toSecureA2a(BaseAgent agent, int port, String service, WeilAction... extra)
        throws IOException {
        secureA2aServer(agent, service, extra).port(port).build().start();
    }

    /**
     * As {@link #secureA2aServer(BaseAgent, String, WeilAction...)}, for an ADK
     * {@link App}: its root agent is served with the App's settings (context
     * caching, event compaction, resumability) and plugins kept, the Weil
     * plugin first.
     */
    public static WeilAgentServer.Builder secureA2aServer(
        App app, String service, WeilAction... extra) {
        return WeilAgentServer.builder(app, identity())
            .requirePermission(service)
            .sentinelHost(System.getenv("SENTINEL_HOST"))
            .plugins(new ArrayList<Plugin>(plugins(extra)));
    }

    /**
     * As {@link #secureA2aServer(App, String, WeilAction...)}, callers checked
     * on WEIL_SERVICE.
     */
    public static WeilAgentServer.Builder secureA2aServer(App app, WeilAction... extra) {
        return secureA2aServer(app, System.getenv("WEIL_SERVICE"), extra);
    }

    /** Serve an ADK {@link App} over A2A on {@code port}, callers checked on {@code service}. */
    public static void toSecureA2a(App app, int port, String service, WeilAction... extra)
        throws IOException {
        secureA2aServer(app, service, extra).port(port).build().start();
    }

    /** Serve an ADK {@link App} over A2A on {@code port}, callers checked on WEIL_SERVICE. */
    public static void toSecureA2a(App app, int port, WeilAction... extra) throws IOException {
        secureA2aServer(app, extra).port(port).build().start();
    }

    /**
     * Ask {@code agent} the question in {@code args}, or else each of
     * {@code defaults} in turn, in one session, printing the answers; then
     * flush and close.
     *
     * <p>With {@code -i} (or {@code --interactive}) as the first argument, the
     * questions are typed instead: each line read at the {@code >} prompt is
     * asked in the same session, until {@code exit}, {@code quit} or the end
     * of input (Ctrl+D).</p>
     */
    public static void chat(BaseAgent agent, String[] args, String... defaults) {
        chat(agent, plugins(), args, defaults);
    }

    /** As {@link #chat(BaseAgent, String[], String...)}, with plugins of the caller's own. */
    public static void chat(
        BaseAgent agent, List<? extends Plugin> plugins, String[] args, String... defaults) {
        if (interactive(args)) {
            converse(() -> new InMemoryRunner(agent, agent.name(), plugins), agent.name(),
                plugins, typed(), false);
            return;
        }
        chat(agent, plugins,
            args.length > 0 ? List.of(String.join(" ", args)) : List.of(defaults));
    }

    /**
     * Ask {@code agent} each of {@code questions} in one session, printing the
     * answers; then flush and close. The questions are read one at a time, so
     * they may be typed as the conversation goes.
     */
    public static void chat(
        BaseAgent agent, List<? extends Plugin> plugins, Iterable<String> questions) {
        converse(() -> new InMemoryRunner(agent, agent.name(), plugins), agent.name(),
            plugins, questions, true);
    }

    /**
     * As {@link #chat(BaseAgent, String[], String...)}, for an ADK {@link App}:
     * the App's settings (context caching, event compaction, resumability)
     * and plugins are kept, with the Weil plugin run first. {@code -i} works
     * here too.
     */
    public static void chat(App app, String[] args, String... defaults) {
        List<BasePlugin> plugins = plugins();
        boolean interactive = interactive(args);
        converse(() -> Runner.builder().app(withPlugins(app, app.name(), plugins)).build(),
            app.name(), plugins,
            interactive ? typed()
                : args.length > 0 ? List.of(String.join(" ", args)) : List.of(defaults),
            !interactive);
    }

    /** Whether {@code args} ask for the questions to be typed: {@code -i} or {@code --interactive} first. */
    private static boolean interactive(String[] args) {
        return args.length > 0 && ("-i".equals(args[0]) || "--interactive".equals(args[0]));
    }

    /**
     * The lines typed at the terminal, each after a {@code >} prompt, read
     * one at a time as the conversation goes; blank lines are skipped, and
     * {@code exit}, {@code quit} or the end of input stops it.
     */
    private static Iterable<String> typed() {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        System.out.println("Type a message for the agent; exit, quit or Ctrl+D to stop.");
        return () -> new Iterator<>() {
            private String next;
            private boolean done;

            @Override
            public boolean hasNext() {
                while (next == null && !done) {
                    System.out.println();
                    System.out.print("> ");
                    System.out.flush();
                    String line;
                    try {
                        line = in.readLine();
                    } catch (IOException e) {
                        throw new UncheckedIOException(e);
                    }
                    if (line == null || "exit".equalsIgnoreCase(line.trim())
                        || "quit".equalsIgnoreCase(line.trim())) {
                        done = true;
                    } else if (!line.isBlank()) {
                        next = line.trim();
                    }
                }
                return next != null;
            }

            @Override
            public String next() {
                if (!hasNext()) {
                    throw new NoSuchElementException();
                }
                String line = next;
                next = null;
                return line;
            }
        };
    }

    /**
     * Serve {@code agent} in ADK's dev UI, the web page {@code adk web} opens,
     * with the Weil plugin on every run: at http://localhost:8080, or on
     * {@code PORT} and {@code SERVER_ADDRESS} when set, as a container sets
     * them ({@code 0.0.0.0}). {@code args} go to Spring Boot, so
     * {@code --server.port=9000} works too. Returns once the server is up; it
     * keeps serving until the process stops.
     *
     * <p>Needs {@code com.google.adk:google-adk-dev}, the same version as
     * {@code google-adk}, which the SDK does not bring itself, so agents that
     * never open the UI do not carry it.</p>
     */
    public static void web(BaseAgent agent, String[] args, WeilAction... extra) {
        WeilWeb.requireDevUi();
        WeilWeb.serve(args, plugins(extra), agent);
    }

    /**
     * As {@link #web(BaseAgent, String[], WeilAction...)}, with plugins of the
     * caller's own instead of {@link #plugins}: a {@link WeilPlugin} built by
     * hand, say. Such a plugin is shut down when the process stops, so a
     * queued one sends what it still holds.
     */
    public static void web(BaseAgent agent, List<? extends BasePlugin> plugins, String[] args) {
        WeilWeb.requireDevUi();
        for (BasePlugin plugin : plugins) {
            if (plugin instanceof WeilPlugin weil && !Weil.plugins.contains(weil)) {
                Runtime.getRuntime().addShutdownHook(new Thread(weil::shutdown, "weil-plugin-shutdown"));
            }
        }
        WeilWeb.serve(args, new ArrayList<>(plugins), agent);
    }

    /**
     * As {@link #web(BaseAgent, String[], WeilAction...)}, for an ADK
     * {@link App}: the App's settings (context caching, event compaction,
     * resumability) and plugins are kept, with the Weil plugin run first.
     */
    public static void web(App app, String[] args, WeilAction... extra) {
        WeilWeb.requireDevUi();
        WeilWeb.serve(args, plugins(extra), app);
    }

    /**
     * {@code app} renamed {@code name}, with {@code first} run before its own
     * plugins and every other setting kept.
     */
    static App withPlugins(App app, String name, List<? extends Plugin> first) {
        List<Plugin> all = new ArrayList<>(first);
        all.addAll(app.plugins());
        App.Builder copy = App.builder()
            .name(name)
            .rootAgent(app.rootAgent())
            .plugins(all)
            .eventsCompactionConfig(app.eventsCompactionConfig())
            .contextCacheConfig(app.contextCacheConfig());
        if (app.resumabilityConfig() != null) {
            copy.resumabilityConfig(app.resumabilityConfig());
        }
        return copy.build();
    }

    /**
     * Ask each question in one session of the runner, printing the answers;
     * then flush and close. {@code echo} prints each question first, which a
     * typed one, already on screen, does not need.
     */
    private static void converse(
        Supplier<Runner> runners, String appName,
        List<? extends Plugin> plugins, Iterable<String> questions, boolean echo) {
        try {
            Runner runner = runners.get();
            Session session = runner.sessionService()
                .createSession(appName, "user")
                .blockingGet();
            for (String question : questions) {
                if (echo) {
                    System.out.println();
                    System.out.println("> " + question);
                }
                System.out.println();
                runner.runAsync(session.userId(), session.id(),
                        Content.fromParts(Part.fromText(question)))
                    .blockingForEach(event -> {
                        String text = event.stringifyContent();
                        if (text != null && !text.isBlank()) {
                            System.out.println(text);
                        }
                    });
            }
        } finally {
            for (Plugin plugin : plugins) {
                if (plugin instanceof WeilPlugin weil && !Weil.plugins.contains(weil)) {
                    weil.shutdown();
                }
            }
            shutdown();
        }
    }

    /**
     * Another Weil agent, served over A2A at {@code url}, as a tool the model
     * calls with a question: each call is signed with this wallet, so that
     * agent can check its permission.
     */
    public static BaseTool remoteTool(String name, String url, String description) {
        return WeilA2A.remoteTool(name, url, identity(), description);
    }

    /**
     * Another Weil agent, served over A2A at {@code url}, as a sub-agent the
     * conversation is handed to, every call signed with this wallet.
     */
    public static BaseAgent remoteAgent(String name, String url, String description) {
        return WeilA2A.remoteAgent(name, url, identity(), description);
    }

    /**
     * {@code agent} as a tool another agent in this process calls, run with
     * the caller's plugins: its own tools' {@code @WeilSecured} gates apply,
     * and its steps are audited and traced. ADK's {@code AgentTool.create}
     * leaves the plugins out, and the Weil plugin refuses such a tool when
     * its agent holds secured tools.
     */
    public static AgentTool agentTool(BaseAgent agent) {
        return AgentTool.create(agent, /* skipSummarization= */ false, /* includePlugins= */ true);
    }

    /**
     * As {@link #agentTool(BaseAgent)}, callable only by a wallet holding
     * Execution or Management in {@code service}, such as
     * {@code "finance.weil"}: the agent-level check, carried by the tool as
     * {@code @WeilSecured} is by a method, so no gate is matched by name.
     * The wallet is the A2A caller's on a served agent, else the agent's own;
     * refused, the agent never starts and the model is told why.
     */
    public static AgentTool agentTool(BaseAgent agent, String service) {
        return new SecuredAgentTool(agent, service);
    }

    /** The graph-memory tools over this wallet's memory. */
    public static List<BaseTool> memory() {
        return PensieveTools.all(identity());
    }

    /** {@code tools} and then the graph-memory tools, as one list for an agent. */
    public static List<BaseTool> withMemory(BaseTool... tools) {
        List<BaseTool> all = new ArrayList<>(List.of(tools));
        all.addAll(memory());
        return all;
    }

    /**
     * Send every record still queued by the plugins made here, however long
     * that takes, then close the wallet, and say how the records went. Runs
     * on its own when a chat ends and when the process stops (Ctrl+C, a
     * container stop); once is enough, and again does nothing. A process
     * killed outright loses what was still queued.
     */
    public static synchronized void shutdown() {
        if (shutDown) {
            return;
        }
        shutDown = true;
        for (WeilPlugin plugin : plugins) {
            Dispatcher queue = plugin.dispatcher();
            long left = queue.pending();
            if (left > 0) {
                System.out.println("  [weil] sending " + left + " queued record(s)...");
            }
            plugin.shutdown();
            System.out.println("  [weil] records: " + queue.summary());
        }
        if (identity != null) {
            identity.close();
        }
    }

    /**
     * The env's wallet, deferred: nothing is read from Sentinel or disk until
     * something needs it, so a server starts, and answers a health check, even
     * when the key is a placeholder. A missing or bad key fails that first use.
     */
    private static WeilIdentity loadIdentity() {
        String apiKey = System.getenv("WEIL_AGENT_KEY");
        String host = System.getenv("SENTINEL_HOST");
        if (apiKey != null && !apiKey.isBlank()) {
            return host != null && !host.isBlank()
                ? WeilIdentity.deferred(() -> WeilClient.fromApiKey(apiKey, null, host), host)
                : WeilIdentity.deferred(() -> WeilClient.fromApiKey(apiKey), null);
        }
        String walletFile = System.getenv("WEIL_WALLET_FILE");
        return WeilIdentity.deferred(() -> {
            if (walletFile == null || walletFile.isBlank()) {
                throw new IllegalStateException(
                    "set WEIL_AGENT_KEY (Agent Registry API key) or WEIL_WALLET_FILE");
            }
            return WeilClient.fromWalletFile(walletFile);
        }, null);
    }
}
