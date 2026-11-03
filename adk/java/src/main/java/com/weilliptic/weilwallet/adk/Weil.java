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

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
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
 * Weil.chat(agent, args, "a question");        // or ask it at the terminal
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
 * annotation names. Queued writes are flushed when a chat ends and, for a
 * served agent, when the process stops.</p>
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

    private Weil() {}

    /**
     * The agent's wallet, from WEIL_AGENT_KEY (with SENTINEL_HOST when set)
     * or WEIL_WALLET_FILE, loaded on first use. Stopping the process flushes
     * and closes it.
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
     */
    public static synchronized WeilPlugin plugin(WeilAction... extra) {
        WeilIdentity id = identity();
        WeilPlugin plugin = WeilPlugin.builder(id)
            .name("weil-audit")
            .withActions(AuditAction.withContent())
            // One trace per run, published to "audits" when it ends. Best
            // effort: a trace that cannot be written never stops the agent.
            .withActions(TraceAction.actions())
            .withActions(PermissionGate.annotated(id, System.getenv("SENTINEL_HOST")))
            .withActions(extra)
            .flushAfterRun(60_000L)
            .onError((ev, action, err) ->
                System.out.println("  [weil] " + action + " on " + ev.hook() + ": " + err))
            .build();
        plugins.add(plugin);
        return plugin;
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

    /**
     * The port an agent serves on: {@code PORT} from the environment, as
     * containers and cloud platforms set it, else {@code fallback}. The
     * address it listens on and the URL its card names come from
     * {@code WEIL_A2A_BIND} and {@code WEIL_A2A_PUBLIC_URL}.
     */
    public static int port(int fallback) {
        return WeilAgentServer.portFromEnv(fallback);
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
     */
    public static void chat(BaseAgent agent, String[] args, String... defaults) {
        chat(agent, plugins(), args, defaults);
    }

    /** As {@link #chat(BaseAgent, String[], String...)}, with plugins of the caller's own. */
    public static void chat(
        BaseAgent agent, List<? extends Plugin> plugins, String[] args, String... defaults) {
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
            plugins, questions);
    }

    /**
     * As {@link #chat(BaseAgent, String[], String...)}, for an ADK {@link App}:
     * the App's settings (context caching, event compaction, resumability)
     * and plugins are kept, with the Weil plugin run first.
     */
    public static void chat(App app, String[] args, String... defaults) {
        List<BasePlugin> plugins = plugins();
        converse(() -> Runner.builder().app(withPlugins(app, app.name(), plugins)).build(),
            app.name(), plugins,
            args.length > 0 ? List.of(String.join(" ", args)) : List.of(defaults));
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

    /** Ask each question in one session of the runner, printing the answers; then flush and close. */
    private static void converse(
        Supplier<Runner> runners, String appName,
        List<? extends Plugin> plugins, Iterable<String> questions) {
        try {
            Runner runner = runners.get();
            Session session = runner.sessionService()
                .createSession(appName, "user")
                .blockingGet();
            for (String question : questions) {
                System.out.println();
                System.out.println("> " + question);
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
     * Flush every plugin made here and close the wallet. Runs on its own when
     * a chat ends and when the process stops; once is enough, and again does
     * nothing.
     *
     * <p>The flush only matters for a plugin given a queued {@link Dispatcher}:
     * with the default inline one every record was sent before its hook
     * returned, so there is nothing left to wait for.</p>
     */
    public static synchronized void shutdown() {
        if (shutDown) {
            return;
        }
        shutDown = true;
        for (WeilPlugin plugin : plugins) {
            plugin.shutdown();
        }
        if (identity != null) {
            identity.close();
        }
    }

    private static WeilIdentity loadIdentity() {
        String apiKey = System.getenv("WEIL_AGENT_KEY");
        String host = System.getenv("SENTINEL_HOST");
        try {
            if (apiKey != null && !apiKey.isBlank()) {
                return host != null && !host.isBlank()
                    ? WeilIdentity.fromApiKey(apiKey, host)
                    : WeilIdentity.fromApiKey(apiKey);
            }
            String walletFile = System.getenv("WEIL_WALLET_FILE");
            if (walletFile == null || walletFile.isBlank()) {
                throw new IllegalStateException(
                    "set WEIL_AGENT_KEY (Agent Registry API key) or WEIL_WALLET_FILE");
            }
            return WeilIdentity.fromWalletFile(walletFile);
        } catch (IOException e) {
            throw new IllegalStateException("cannot load the agent's wallet: " + e.getMessage(), e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("interrupted loading the agent's wallet", e);
        }
    }
}
