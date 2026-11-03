# Running, plugging in and securing a Weil ADK agent

Every way the Java SDK runs an agent, attaches the Weil plugin to it, secures
it, and connects it to other agents, each with when to use it and the example
that does it. Paths are under `src/main/java/com/weilliptic/weilwallet/`
unless they start with `examples-web/`.

The three questions are independent: pick a way to **run** (§1), a way to
**attach the plugin** (§2), and the **gates** you need (§3). Most agents use
the defaults in each, and need two lines of Weil code.

| I want to... | Use | Section |
|---|---|---|
| Try an agent in the terminal | `Weil.chat(agent, args, "question")` | 1.1 |
| Expose an agent to other agents | `Weil.toSecureA2a(agent, port, "engg.weil")` | 1.4 |
| Chat with it in a browser | ADK dev UI with the `extraPlugins` bean | 1.7 |
| Define it in YAML, not Java | `WeilYamlAgent [--serve port] file.yaml` | 1.8 |
| Lock a tool I wrote | `@WeilSecured("engg.weil")` on its method | 3.2 |
| Lock tools I did not write (MCP, libraries) | `PermissionGate.on(svc, id, host, filter)` | 3.3 |
| Lock who may call my agent | the service in `toSecureA2a`, or `WEIL_SERVICE` | 3.1 |
| Call another agent | `Weil.remoteTool(...)` | 4 |

Everything reads its settings from the env file: `WEIL_AGENT_KEY` (or
`WEIL_WALLET_FILE`) and `SENTINEL_HOST` for the agent's wallet, the model
settings, and optionally `WEIL_SERVICE`. `Weil.identity()` loads the wallet
once and closes it when the process stops.

---

## 1. Ways to run an agent

### 1.1 Terminal chat: `Weil.chat(agent, args, defaults...)`

```java
Weil.chat(agent, args, "Roll a 6-sided die and check if it is prime.");
```

Runs the agent in one session: the question in `args`, or else each default
in turn, printing the answers, then shuts the plugin down (§2.6) and exits. The
Weil plugin (§2.1) is attached for you.

**Use when:** a demo, a script, or a single agent you are developing.
**Examples:** `examples/helloworld/HelloWorldAgent.java`,
`examples/city_time_weather/CityTimeWeather.java`,
`examples/WeilTravelPlanner.java`.

### 1.2 Terminal chat with your own plugin list: `Weil.chat(agent, plugins, args, defaults...)`

```java
Weil.chat(agent, Weil.plugins(filesGate), args, "...");   // the Weil plugin plus a gate
Weil.chat(agent, List.of(myPlugin), args, "...");         // a plugin you built (§2.4)
```

**Use when:** the agent needs extra gates (§3.3), or a plugin you built
yourself. **Examples:** `examples/mcpfilesystem/McpFilesystemAgent.java`,
`examples/financialadvisor/FinancialAdvisorAgent.java`,
`examples/WeilAdkPluginExample.java`.

### 1.3 An ADK `App`: `Weil.chat(app, ...)` / `Weil.toSecureA2a(app, port[, service])`

```java
Weil.chat(App.builder().name("cache_demo").rootAgent(agent)
    .contextCacheConfig(...).build(), args, PROMPTS);
```

The App's own settings (context caching, event compaction, resumability) and
its own plugins are kept; the Weil plugin runs first.

**Use when:** the agent needs App-level settings. **Example:**
`examples/contextcaching/ContextCachingAgent.java`.

### 1.4 Serve over A2A: `Weil.toSecureA2a(agent, port[, service])`

```java
Weil.toSecureA2a(agent, 8001, "engg.weil");   // callers checked on engg.weil
Weil.toSecureA2a(agent, 8001);                // callers checked on WEIL_SERVICE, open when unset
Weil.toSecureA2a(agent, 8001, "engg.weil", extraGate);
```

Starts an A2A server: agent card at `/.well-known/agent-card.json`, JSON-RPC
at `/`. Each request's signature is verified (401 if it fails), then the
caller's permission (§3.1, 403 if refused), and only then does the agent run,
with the Weil plugin.

**Use when:** other agents, in any language with the Weil signer, should call
this one. **Examples:** `examples/a2a_server/agent/Agent.java`,
`examples/WeilTravelFlights.java`, `examples/WeilTravelHotels.java`
(`./run.sh flights`, `./run.sh hotels`).

### 1.5 Serve over A2A with more options: `Weil.secureA2aServer(...)` or `WeilAgentServer.builder(...)`

```java
Weil.secureA2aServer(agent, "engg.weil")
    .port(8001)                                   // else PORT, else 8001
    .bindAddress("0.0.0.0")                       // else WEIL_A2A_BIND, else 127.0.0.1
    .publicUrl("https://flights.example.com")   // the URL the card advertises; else WEIL_A2A_PUBLIC_URL
    .agentTimeoutSeconds(300)
    .authorize(caller -> allowed(caller.canonicalAddress()) ? null : "not on the list")
    .publishPermissionChecks(false)               // no verdicts on the security topic
    .build()
    .start();
```

`secureA2aServer` returns the builder `toSecureA2a` would have started, with the
wallet, the service, the Sentinel host and the Weil plugin already set.
`WeilAgentServer.builder(agent, identity)` is the same builder with nothing set:
pass `requirePermission`, `sentinelHost` and `plugins` yourself.

**Use when:** you need a public URL, a timeout, a caller check of your own, or
a wallet other than the env's. **Example:** `examples/WeilYamlAgent.java`
(serving a YAML agent with `WeilAgentServer.builder`).

### 1.6 Your own runner: `Weil.runner(agent)` or `new InMemoryRunner(agent, name, Weil.plugins())`

```java
InMemoryRunner runner = Weil.runner(agent);                          // plugin attached
Runner runner = new InMemoryRunner(agent, "my-app", Weil.plugins(gate));
runner.runAsync(userId, sessionId, Content.fromParts(Part.fromText("..."))).blockingForEach(...);
```

You own the sessions and the event loop. With the default inline delivery,
every record is sent before its step goes on, so there is nothing to flush; with
a queued dispatcher, call `Weil.shutdown()` at the end, or let the shutdown hook
do it (§2.6).

**Use when:** the agent is part of a larger service (an HTTP handler, a job, a
test) that drives runs itself.
**Example:** `examples-web/.../examples/web/spring_chat/` returns the runner as
a Spring bean behind `POST /api/chat` (`./run.sh spring-chat`), and
`.../spring_chat_advanced/` streams each run's events as server-sent events
(`./run.sh spring-chat-advanced`).

### 1.7 ADK's dev web UI

`AdkWebServer.start(...)` takes agents but no plugins, so start ADK's Spring
app yourself and register two beans:

```java
System.setProperty("adk.agents.loader", "static");
SpringApplication app = new SpringApplication(AdkWebServer.class);
app.addInitializers(ctx -> {
    var beans = (DefaultListableBeanFactory) ctx.getBeanFactory();
    beans.registerSingleton("agentLoader", new AgentStaticLoader(planner));
    beans.registerSingleton("extraPlugins", Weil.plugins());   // without it: no audit, no trace
});
app.run(args);
```

Each message in the UI is its own run, so it gets its own audit records and its
own trace. A web server never "ends"; with a queued dispatcher the plugin would
be flushed when it stops (§2.6), and with the default inline one nothing waits.
The UI needs `google-adk-dev` (Spring Boot, Tomcat), so it lives in its own
module.

**Use when:** you want to chat over many turns and inspect every event.
**Example:** `examples-web/.../examples/web/WeilTravelPlannerWeb.java`
(`./run.sh web`, then http://127.0.0.1:8080).

### 1.8 A YAML agent: `WeilYamlAgent`

```bash
mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilYamlAgent \
  -Dexec.args="[--serve 8001] path/to/root_agent.yaml [question]"
```

Loads an ADK Agent Config file with the env's wallet and runs it as 1.2
(chat) or, with `--serve`, as 1.5 (A2A server). YAML cannot call Java, so Weil
registers tool names for it: `weil_memory` (as `Weil.memory()`) and
`weil_remote_agent` with `name`, `url` and `description` (as
`Weil.remoteTool`). Security is the file's `weil:` block (§3.4).

**Use when:** agents are defined as config, edited without recompiling.
**Examples:** `examples/configagent/` (every ADK YAML sample, plus `dice/` and
`travel/`); see its README.

### 1.9 From YAML to Java: `WeilYamlToJava`

Writes the Java class that builds the same agent tree as a YAML file,
sub-agents and `weil:` gates included, with a `main` that runs it as 1.8 does.

**Use when:** an agent prototyped in YAML needs custom code. See "Turning a
YAML agent into Java" in `examples/configagent/README.md`.

---

## 2. Ways to attach the Weil plugin

### 2.1 What the standard plugin holds

`Weil.plugin(extra...)` builds one `WeilPlugin` for the env's wallet:

| In order | What it does |
|---|---|
| `AuditAction.withContent()` | A record on `auditor::weil` at every step, with the prompt, tool arguments and results, and model replies |
| `TraceAction.actions()` | One trace per run on `weilchain::weil`: a graph of tool calls, published as a Mermaid flowchart to `publisher::<org>`, topic `audits` |
| `PermissionGate.annotated(...)` | Gates every tool whose method carries `@WeilSecured` (§3.2) |
| `extra` | Whatever you pass: usually `PermissionGate`s (§3.3) |

Its records are sent inline: each is accepted by Sentinel, with up to three
attempts, before the step goes on, so none is lost when the process stops. A
failure is printed as `[weil] <action> on <hook>: <error>` and never stops the
agent. It also sets `flushAfterRun(60 s)`, which only matters if you give a
plugin a queued dispatcher (§2.6).

`Weil.plugins(extra...)`, which every way of running in §1 uses, adds one more
plugin beside it: `MemoryPrompt.plugin()`. Before each model call of an agent
that holds the memory tools (`Weil.withMemory`, `Weil.memory`), it appends the
memory instructions (`memory-prompt.txt`, the same as icarus uses: read memory
first, store only the facts and preferences the user states, as entities and
relations) to that agent's system instruction, dated that
day. Agents without memory are left alone, and an instruction that already
carries the text by hand is not given it twice. So an agent with memory needs
no instruction of its own about it.

### 2.2 Implicitly: `Weil.chat`, `Weil.toSecureA2a`, `Weil.secureA2aServer`, `Weil.runner`

These attach `Weil.plugins(extra)` themselves (the Weil plugin and the memory
prompt); pass extra gates as their last arguments. Nothing else to do.

### 2.3 As a list: `Weil.plugins(extra...)`

The standard plugin and the memory prompt as the `List<BasePlugin>` that ADK's
runners, `Weil.chat`'s plugin overload, and the dev UI's `extraPlugins` bean
take. To combine them with plugins of your own, build the list from the parts:
`List.of(Weil.plugin(gates), MemoryPrompt.plugin(), myPlugin)`. Leave
`MemoryPrompt.plugin()` out to write your own memory instructions instead.

### 2.4 Built by hand: `WeilPlugin.builder(identity)`

```java
WeilPlugin plugin = WeilPlugin.builder(identity)
    .name("weil-audit")
    .withActions(AuditAction.defaults(),               // digests only, no content
        PermissionGate.annotated(identity, sentinelHost),
        spendCap)                                       // an action of your own
    .dispatcher(new Dispatcher())                       // optional: a background queue (default: inline)
    .flushAfterRun(60_000L)                             // only with a queued dispatcher (§2.6)
    .onError((ev, action, err) -> log.warn(...))
    .build();
Weil.chat(agent, List.of(plugin, MemoryPrompt.plugin()), args, "...");   // the prompt, if the agent has memory
```

**Use when:** you want a different set of actions (no trace, digest-only
audits, a stream publish, a custom check), or a wallet other than the env's.
**Example:** `examples/WeilAdkPluginExample.java` (a spending cap read from
chain state). HOOKS.md §9 shows how to write an action.

### 2.5 Which agents one plugin covers

| Agent | Covered by the runner's plugin? |
|---|---|
| The root agent and its `subAgents` (transfer) | Yes: one plugin covers the whole tree in the run |
| A local agent behind `Weil.agentTool(agent)` (or `AgentTool.create(agent, false, true)`) | Yes: it is handed the plugins; its run is a `subtrace` in the caller's trace |
| A local agent behind `AgentTool.create(agent)` | **No**: ADK leaves plugins out, so it is not audited, gated or traced. If its agent holds `@WeilSecured` tools, the plugin refuses the call instead of letting them run unchecked (§3.2) |
| A remote agent (`remoteTool` / `remoteAgent`) | No: it runs in its own process with its own plugin and wallet |

### 2.6 `flush` and `shutdown`: only with a queued dispatcher

How a non-blocking record (an audit, a verdict) reaches Sentinel depends on the
plugin's `Dispatcher`:

| | Inline (default everywhere) | Queued (`new Dispatcher(...)`) |
|---|---|---|
| A record is sent | on the agent's thread, before the step goes on | later, by a background worker |
| When the process exits | nothing is pending | records still queued die with the workers |
| `flush` / `shutdown` | return at once: nothing to do | **needed**, or the last records of a run are lost |

**`flush(ms)`**, on the dispatcher, waits until the queue is empty and no worker
is sending, or until the time is up. `flushAfterRun(ms)` makes the plugin call it
at the end of every run; `Weil.plugin()` sets 60 s.

**`shutdown()`**, on the plugin, flushes for up to 10 s and then stops the
worker threads. `Weil.shutdown()` does that for every plugin `Weil` made and
closes the wallet. It runs on its own at the end of `Weil.chat` and, through a
JVM shutdown hook, when the process exits normally or on Ctrl-C (not on
`kill -9` or a crash).

With inline delivery, the default, none of this needs calling. With a queued
dispatcher, a terminal run looks after itself through `Weil.chat`; code that
drives its own runner calls `shutdown()` itself:

```java
Dispatcher queue = new Dispatcher(4, 1_000, Dispatcher.Overflow.WAIT, 30_000L);
WeilPlugin plugin = WeilPlugin.builder(Weil.identity())
    .withActions(AuditAction.withContent())
    .dispatcher(queue)              // the agent no longer waits for its records
    .flushAfterRun(30_000L)         // each run ends only once its records are sent
    .build();

InMemoryRunner runner = new InMemoryRunner(agent, "batch", List.of(plugin));
try {
    for (String question : questions) {
        // run each question; records go out in the background meanwhile
    }
} finally {
    plugin.shutdown();              // send what is still queued (up to 10 s)
    queue.close();                  // stop its workers: a dispatcher you pass in is yours to close
}
```

A plugin built by hand is not one `Weil` made, so `Weil.shutdown()` and the
shutdown hook do not cover it: the `finally` is what sends its last records.

A served agent or the web UI never "ends a run" for good, so there the shutdown
hook is what sends the last records when the server is stopped.

---

## 3. Ways to secure an agent

Two levels, checked at different times:

- **Agent level**, on an A2A server: may this caller run the agent at all?
  Checked before the agent's model runs, so a refused caller costs nothing.
- **Tool level**, in the plugin: may this wallet run this tool? Checked just
  before the tool runs.

Both ask the chain whether a wallet holds **Execution** or **Management**
purpose (or **Management** only, for admin) in a service written
`"<subgroup>.<org>"`, such as `engg.weil`. The wallet checked is the A2A
caller's on a served agent, else the agent's own. Every verdict is published
to the `security` topic on `publisher::<org>`.

### 3.1 Agent level: who may call

| Way | Written as | Use when |
|---|---|---|
| Service in code | `Weil.toSecureA2a(agent, port, "engg.weil")` | The agent always belongs to one service (recommended) |
| Service from the env | `Weil.toSecureA2a(agent, port)` with `WEIL_SERVICE=engg.weil` | The same code is deployed into different services |
| Open | `WEIL_SERVICE` unset and no service, or `agent: open` in YAML | Any valid signature may call; tools can still be gated |
| YAML | `weil: agent: secured` (uses `WEIL_SERVICE`), or `agent: {service: engg.weil}` | A YAML agent (§3.4) |
| A check of your own | `.authorize(caller -> reasonOrNull)` on the server builder (§1.5) | An allow-list, a rate limit, or any rule the chain does not hold; runs after the permission check |

A refused caller gets HTTP 403 with one sentence its model can pass on:
`wallet <address> is not authorised to perform operations on <agent>: ...`.
A permission lookup that cannot be made is 503, not 403.

### 3.2 Tool level, tools you wrote: `@WeilSecured`

```java
@WeilSecured("engg.weil")                            // Execution or Management
public static Map<String, Object> searchFlights(...) { ... }

@WeilSecured(value = "finance.weil", admin = true)   // Management only
public static Map<String, Object> payInvoice(...) { ... }
```

The standard plugin's `PermissionGate.annotated` reads the annotation off the
method behind each `FunctionTool` just before it runs. Nothing else to wire.
A plugin built by hand needs `PermissionGate.annotated(identity, sentinelHost)`
in its actions.

Inside another agent called as a tool, the annotation is only read if that
agent runs with the plugin. Make such tools with `Weil.agentTool(agent)`, which
hands it the plugins. ADK's own `AgentTool.create(agent)` leaves them out; when
the agent behind it holds `@WeilSecured` tools, the gate refuses the call with
a message naming them, rather than let them run unchecked.

**Use when:** the tool is a Java method of yours. **Examples:**
`examples/helloworld/HelloWorldAgent.java`, `examples/WeilTravelFlights.java`.

### 3.3 Tool level, tools without a method to annotate: `PermissionGate.on` / `PermissionGate.admin`

```java
// every MCP tool, Execution or Management in engg.weil
PermissionGate.on("engg.weil", Weil.identity(), System.getenv("SENTINEL_HOST"),
    ev -> ev.tool() instanceof AbstractMcpTool<?>);

// only the tools that change files, Management only
PermissionGate.admin("engg.weil", Weil.identity(), System.getenv("SENTINEL_HOST"),
    ev -> Set.of("write_file", "edit_file", "move_file").contains(ev.toolName()));

// one tool by name, each on its own service
PermissionGate.on("finance.weil", Weil.identity(), sentinelHost,
    ev -> "trading_analyst".equals(ev.toolName()));
```

Pass the gate to `Weil.chat`, `Weil.toSecureA2a`, `Weil.plugins` or
`Weil.plugin` as an extra action. The filter decides which tool calls the gate
covers: by type, by name, or anything else on the `WeilEvent`.
`PermissionGate.on(svc, identity)` with no filter gates **every** tool,
memory tools included.

**Use when:** the tools come from an MCP server, a library, a toolset or a
remote agent; or one rule covers many tools; or the rule is decided at runtime.
**Example:** `examples/mcpfilesystem/McpFilesystemAgent.java` (every MCP tool).

For a local agent called as a tool, there is no name to match:
`Weil.agentTool(agent, "finance.weil")` makes the tool carry its service, as a
method carries `@WeilSecured`. The standard plugin checks it before the agent
starts, and the agent runs with the plugins, so its own `@WeilSecured` tools are
checked too. `Weil.agentTool(agent)` without a service only does the latter.
**Example:** `examples/financialadvisor/FinancialAdvisorAgent.java` (each
specialist on its own service).

### 3.4 Tool and agent level in YAML: the `weil:` block

```yaml
weil:
  service: engg.weil            # this file's tools
  agent: secured                # served: callers checked on WEIL_SERVICE
  tools:
    roll_die: user              # Execution or Management
    approve_invoice:
      access: admin             # Management only
      service: finance.acme
```

ADK ignores the block; `WeilYamlAgent` turns it into the server's service
(§3.1) and `PermissionGate`s (§3.3). A tool whose method carries
`@WeilSecured` keeps that gate too: the file can add a rule, never remove one.
Full rules: "Permissions" in `examples/configagent/README.md`.

### 3.5 What a refusal looks like

| Refused at | The caller sees |
|---|---|
| Agent (A2A) | HTTP 403; through `Weil.remoteTool`, the calling model gets `{"error": "flight_agent could not answer: HTTP 403: wallet ... is not authorised ..."}` |
| Tool | The tool does not run; the model gets `{"error": "Permission denied: wallet ... does not have permission to run <tool>. It needs ... permission in <svc>."}` and explains it |
| Gate could not check | The tool is refused (gates fail closed) |

Every refusal is audited and its verdict published to `security`.

---

## 4. Ways to call another agent

| Way | Where it runs | Control | Signed | Example |
|---|---|---|---|---|
| `Weil.remoteTool(name, url, description)` | another process, over A2A | stays with the caller, who gets the answer as a tool result; several can be called in one turn, and they run at the same time | yes | `examples/WeilTravelPlanner.java` |
| `Weil.remoteAgent(name, url, description)` | another process, over A2A | handed to the remote agent, as a sub-agent | yes | `examples/a2a_basic/A2AAgent.java` |
| `Weil.agentTool(agent[, service])` | the same JVM, with the caller's plugins; with a service, callers need permission there | stays with the caller | no (same wallet) | `examples/financialadvisor/FinancialAdvisorAgent.java` |
| `weil_remote_agent` in YAML | another process, over A2A | as `remoteTool` | yes | `examples/configagent/travel/planner_agent.yaml` |

`remoteTool` returns a refusal, an unreachable agent, or a timeout
(`WEIL_A2A_TIMEOUT_SECONDS`, default 300) to the model as `{"error": ...}`
instead of failing the turn. The remote agent's card is read when the tool is
built, so start it first. For calls outside an agent,
`WeilA2A.client(card, identity)` gives a signing a2a-java client.

---

## 5. The examples, by the way they do each

| Example | Runs as | Plugin | Secured by |
|---|---|---|---|
| `helloworld/HelloWorldAgent` | 1.1 chat | 2.2 implicit | `@WeilSecured` |
| `city_time_weather/CityTimeWeather` | 1.1 chat | 2.2 implicit | `@WeilSecured` |
| `contextcaching/ContextCachingAgent` | 1.3 App chat | 2.2 implicit, App's plugins kept | — |
| `mcpfilesystem/McpFilesystemAgent` | 1.2 chat | 2.3 `Weil.plugins(filesGate)` | `PermissionGate.on`, MCP tools by type |
| `financialadvisor/FinancialAdvisorAgent` | 1.1 chat | 2.2 implicit; specialists via `Weil.agentTool`, which hands them the plugins | `Weil.agentTool(agent, service)` per specialist, `@WeilSecured` inside |
| `WeilAdkPluginExample` | 1.2 chat | 2.4 by hand, with a spend cap | `@WeilSecured` via `annotated` |
| `a2a_server/agent/Agent` | 1.4 A2A server | 2.2 implicit | agent on a service in code, `@WeilSecured` |
| `a2a_basic/A2AAgent` | 1.1 chat | 2.2 implicit | `@WeilSecured` on its local tool; calls the server above with `remoteAgent` |
| `WeilTravelFlights`, `WeilTravelHotels` | 1.4 A2A servers | 2.2 implicit | agent on `engg.weil`, `@WeilSecured` on each search |
| `WeilTravelPlanner` | 1.1 chat | 2.2 implicit | none of its own; calls both with `remoteTool` |
| `examples-web/.../WeilTravelPlannerWeb` | 1.7 dev UI | 2.3 as the `extraPlugins` bean | as `WeilTravelPlanner` |
| `examples-web/.../spring_chat/SpringChatApplication` | 1.6 own runner, a Spring bean behind `POST /api/chat` | 2.2 implicit, via `Weil.runner` | — (no tools but memory) |
| `examples-web/.../spring_chat_advanced/SpringChatAdvancedApplication` | 1.6 own runner, streamed as server-sent events | 2.2 implicit, via `Weil.runner` | `@WeilSecured` on Spring bean methods; `talkToHuman` open |
| `configagent/*` | 1.8 YAML (chat or `--serve`) | 2.3 `Weil.plugin(gates)` | the `weil:` block, plus `@WeilSecured` |

Commands for every one: `ADK_EXAMPLES.md`. Hooks, records, and writing an action
of your own: `HOOKS.md`.
