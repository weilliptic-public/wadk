# Running, plugging in and securing a Weil ADK agent

Every way the Java SDK runs an agent, attaches the Weil plugin, secures it and
connects it to other agents, with the example that does each. Paths are under
`src/main/java/com/weilliptic/weilwallet/` unless they start with
`examples-web/`.

| I want to... | Use | Section |
|---|---|---|
| Chat with an agent in a browser | `Weil.web(agent, args)` | 1.1 |
| Run it in the terminal | `Weil.chat(agent, args, "question")` | 1.2 |
| Expose it to other agents | `Weil.toSecureA2a(agent, port, "engg.weil")` | 1.4 |
| Drive runs from my own code | `Weil.runner(agent)` | 1.6 |
| Lock a tool I wrote | `@WeilSecured("engg.weil")` on its method | 3.2 |
| Lock tools I did not write (MCP, libraries) | `PermissionGate.on(svc, id, host, filter)` | 3.3 |
| Lock who may call my agent | the service in `toSecureA2a`, or `WEIL_SERVICE` | 3.1 |
| Call another agent | `Weil.remoteTool(...)` | 4 |

Settings come from the environment: `WEIL_AGENT_KEY` (or `WEIL_WALLET_FILE`)
and `SENTINEL_HOST` for the wallet, the model settings, and optionally
`WEIL_SERVICE`. The wallet loads in the background, so an agent starts without
waiting for Sentinel.

---

## 1. Ways to run an agent

Each of these attaches the Weil plugin (§2) for you; pass extra gates (§3.3) as
the last arguments.

### 1.1 ADK's dev web UI: `Weil.web`

```java
Weil.web(agent, args);                      // the Weil plugins on every run
Weil.web(agent, args, filesGate);           // plus a gate
Weil.web(app, args);                        // an ADK App (1.3)
Weil.web(agent, List.of(myPlugin), args);   // a plugin you built (2.3)
```

Serves the agent in the page `adk web` opens, at http://localhost:8080 (or
`PORT`; `SERVER_ADDRESS` to listen beyond localhost). Each message is its own
run, with its own audit records and trace. Ctrl+C sends what is still queued.

Needs `google-adk-dev`, the same version as `google-adk`; the SDK leaves it
optional. This project's `pom.xml` runs `exec:java` on the test classpath, the
only scope with both it and ADK's runtime-only jars.

**Examples:** every agent in `examples/` except the A2A servers, and
`examples-web/.../WeilTravelPlannerWeb.java`.

### 1.2 Terminal: `Weil.chat`

```java
Weil.chat(agent, args, "Roll a 6-sided die and check if it is prime.");
Weil.chat(agent, Weil.plugins(filesGate), args, "...");   // with a gate
Weil.chat(agent, List.of(myPlugin), args, "...");         // a plugin you built
```

Asks the question in `args`, or else each default in turn, in one session,
prints the answers, sends the queued records and exits. With `-i` as the first
argument, questions are typed at a `>` prompt until `exit` or Ctrl+D.

**Use when:** a script, a test, or a check without a browser. Swap an example's
`Weil.web` for `Weil.chat` to run it this way.

### 1.3 An ADK `App`

`Weil.web(app, args)`, `Weil.chat(app, args, ...)` and
`Weil.toSecureA2a(app, port[, service])` keep the App's settings (context
caching, event compaction, resumability) and its plugins, with the Weil plugin
first. **Example:** `examples/contextcaching/ContextCachingAgent.java`.

### 1.4 Serve over A2A: `Weil.toSecureA2a`

```java
Weil.toSecureA2a(agent, 8001, "engg.weil");   // callers checked on engg.weil
Weil.toSecureA2a(agent, 8001);                // checked on WEIL_SERVICE; open when unset
```

Agent card at `/.well-known/agent-card.json`, JSON-RPC at `/`. Each request's
signature is checked (401), then the caller's permission (403, §3.1), and only
then does the agent run. Callers can be in any language with the Weil signer.

**Examples:** `examples/a2a_server/agent/Agent.java`,
`examples/WeilTravelFlights.java`, `examples/WeilTravelHotels.java`.

### 1.5 Serve with more options: `Weil.secureA2aServer`

```java
Weil.secureA2aServer(agent, "engg.weil")
    .port(8001)                                   // else PORT, else 8001
    .bindAddress("0.0.0.0")                       // else WEIL_A2A_BIND, else 127.0.0.1
    .publicUrl("https://flights.example.com")   // what the card advertises; else WEIL_A2A_PUBLIC_URL
    .agentTimeoutSeconds(300)
    .authorize(caller -> allowed(caller.canonicalAddress()) ? null : "not on the list")
    .publishPermissionChecks(false)               // no verdicts on the security topic
    .build()
    .start();
```

The builder `toSecureA2a` uses, with the wallet, service and Weil plugin set.
`WeilAgentServer.builder(agent, identity)` is the same with nothing set, for a
wallet other than the env's.

### 1.6 Your own runner: `Weil.runner`

```java
InMemoryRunner runner = Weil.runner(agent);                          // plugin attached
Runner runner = new InMemoryRunner(agent, "my-app", Weil.plugins(gate));
```

You own the sessions and the event loop. Records go out in the background; the
shutdown hook sends what is left when the process stops (§2.4).

**Examples:** `examples-web/.../spring_chat/` (the runner as a Spring bean
behind `POST /api/chat`) and `.../spring_chat_advanced/` (each run streamed as
server-sent events).

---

## 2. The Weil plugin

### 2.1 What it does

`Weil.plugin(extra...)` builds one `WeilPlugin` for the env's wallet:

| In order | What it does |
|---|---|
| `AuditAction.withContent()` | A record on `auditor::weil` at every step: prompt, tool arguments and results, model replies |
| `TraceAction.actions()` | One trace per run on `weilchain::weil`, a graph of tool calls. At the end: `index_trace`, then `get_trace` reads it back through Sentinel and the result is published to `publisher::<org>`, topic `audits` |
| `PermissionGate.annotated(...)` | Gates every tool whose method carries `@WeilSecured` (§3.2) |
| `extra` | Usually `PermissionGate`s (§3.3) |

Audits and traces go on an ordered background queue, one worker, so the agent
waits only for the permission gates. A failure is printed as
`[weil] <action> on <hook>: <error>` and never stops the agent.

`Weil.plugins(extra...)`, which every way in §1 uses, adds
`MemoryPrompt.plugin()`: for an agent holding the memory tools
(`Weil.withMemory`, `Weil.memory`), it adds instructions to use them to the
system instruction, so the agent needs none of its own.

### 2.2 Which agents one plugin covers

| Agent | Covered? |
|---|---|
| The root agent and its `subAgents` | Yes: the whole tree in the run |
| A local agent behind `Weil.agentTool(agent)` | Yes: it is handed the plugins; its run is a `subtrace` node in the caller's trace |
| A local agent behind ADK's `AgentTool.create(agent)` | **No**: not audited, gated or traced. If it holds `@WeilSecured` tools, the call is refused rather than run unchecked |
| A remote agent (`remoteTool`, `remoteAgent`) | No: it runs in its own process, with its own plugin and wallet |

### 2.3 Built by hand: `WeilPlugin.builder`

```java
WeilPlugin plugin = WeilPlugin.builder(identity)
    .withActions(AuditAction.defaults(),               // digests only, no content
        PermissionGate.annotated(identity, sentinelHost),
        spendCap)                                       // an action of your own
    .onError((ev, action, err) -> log.warn(...))
    .build();
Weil.web(agent, List.of(plugin, MemoryPrompt.plugin()), args);
```

**Use when:** a different set of actions (no trace, digest-only audits, a
custom check) or a wallet other than the env's. Keep
`PermissionGate.annotated` in the list, or `@WeilSecured` is not checked.
**Example:** `examples/WeilAdkPluginExample.java`, a spending cap read from
chain state. HOOKS.md §9 shows how to write an action.

### 2.4 When queued records are sent

`Weil.plugin()` sends in the background, so records still queued when a run
ends go out later. Everything left is sent when a `Weil.chat` ends or the
process stops (Ctrl+C, a container stop), through a shutdown hook, and the exit
prints e.g. `[weil] records: 46 sent, 0 failed`. A process killed outright
(`kill -9`) loses what was queued.

A plugin built by hand sends on the agent's thread unless you give it a queued
`Dispatcher`. Then call `plugin.shutdown()` when you are done, or use
`Weil.web` / `Weil.chat`, which do it for you.

---

## 3. Securing an agent

Two levels:

- **Agent level**, on an A2A server: may this caller run the agent at all?
  Checked before the model runs, so a refused caller costs nothing.
- **Tool level**, in the plugin: may this wallet run this tool? Checked just
  before the tool runs.

Both ask the chain whether a wallet holds **Execution** or **Management** (or
**Management** only, for admin) in a subgroup written `"<subgroup>.<org>"`,
such as `engg.weil`. On a served agent the wallet checked is the caller's,
otherwise the agent's own. Every verdict is published to the `security` topic.

### 3.1 Agent level: who may call

| Way | Written as |
|---|---|
| Service in code (recommended) | `Weil.toSecureA2a(agent, port, "engg.weil")` |
| Service from the env | `Weil.toSecureA2a(agent, port)` with `WEIL_SERVICE=engg.weil` |
| Open: any valid signature | no service and `WEIL_SERVICE` unset |
| A rule of your own | `.authorize(caller -> reasonOrNull)` on the builder (§1.5), after the permission check |

A refused caller gets HTTP 403 and a sentence its model can pass on. A
permission lookup that cannot be made is 503.

### 3.2 Tools you wrote: `@WeilSecured`

```java
@WeilSecured("engg.weil")                            // Execution or Management
public static Map<String, Object> searchFlights(...) { ... }

@WeilSecured(value = "finance.weil", admin = true)   // Management only
public static Map<String, Object> payInvoice(...) { ... }
```

Read off the method just before the tool runs. Inside an agent called as a
tool, it is checked only if that agent is wrapped with `Weil.agentTool` (§2.2).
**Examples:** `examples/helloworld/HelloWorldAgent.java`,
`examples/WeilTravelFlights.java`.

### 3.3 Tools without a method to annotate: `PermissionGate`

```java
// every MCP tool, Execution or Management in engg.weil
PermissionGate.on("engg.weil", Weil.identity(), System.getenv("SENTINEL_HOST"),
    ev -> ev.tool() instanceof AbstractMcpTool<?>);

// only the tools that change files, Management only
PermissionGate.admin("engg.weil", Weil.identity(), System.getenv("SENTINEL_HOST"),
    ev -> Set.of("write_file", "edit_file", "move_file").contains(ev.toolName()));
```

Pass the gate to `Weil.web`, `Weil.chat` or `Weil.toSecureA2a`. The filter
picks the tool calls it covers; with no filter it covers every tool, memory
included. **Example:** `examples/mcpfilesystem/McpFilesystemAgent.java`.

For a local agent called as a tool, `Weil.agentTool(agent, "finance.weil")`
gates the whole agent, and its own `@WeilSecured` tools are checked too.
**Example:** `examples/financialadvisor/FinancialAdvisorAgent.java`, each
specialist on its own subgroup.

### 3.4 What a refusal looks like

| Refused at | The caller sees |
|---|---|
| Agent (A2A) | HTTP 403; through `Weil.remoteTool`, the model gets `{"error": "flight_agent could not answer: HTTP 403: wallet ... is not authorised ..."}` |
| Tool | The tool does not run; the model gets `{"error": "Permission denied: wallet ... does not have permission to run <tool> ..."}` and explains it |
| The check could not be made | The tool is refused (gates fail closed) |

---

## 4. Calling another agent

| Way | Where it runs | Control | Example |
|---|---|---|---|
| `Weil.remoteTool(name, url, description)` | another process, over signed A2A | stays with the caller; several can run at once in one turn | `examples/WeilTravelPlanner.java` |
| `Weil.remoteAgent(name, url, description)` | another process, over signed A2A | handed to the remote agent, as a sub-agent | `examples/a2a_basic/A2AAgent.java` |
| `Weil.agentTool(agent[, service])` | the same process, with the caller's plugins | stays with the caller | `examples/financialadvisor/FinancialAdvisorAgent.java` |

A refusal, an unreachable agent or a timeout (`WEIL_A2A_TIMEOUT_SECONDS`,
default 300) reaches the model as `{"error": ...}` instead of failing the turn.
Start remote agents first: their card is read when the tool is built.
`WeilA2A.client(card, identity)` gives a signing a2a-java client for calls
outside an agent.

---

## 5. The examples

| Example | Runs as | Secured by |
|---|---|---|
| `helloworld/HelloWorldAgent` | dev UI | `@WeilSecured` |
| `city_time_weather/CityTimeWeather` | dev UI | `@WeilSecured` |
| `contextcaching/ContextCachingAgent` | dev UI, an App | none |
| `mcpfilesystem/McpFilesystemAgent` | dev UI | `PermissionGate.on`, MCP tools by type |
| `financialadvisor/FinancialAdvisorAgent` | dev UI | `Weil.agentTool(agent, service)` per specialist, `@WeilSecured` inside |
| `WeilAdkPluginExample` | dev UI, its own plugin | `@WeilSecured`, plus a spending cap |
| `a2a_server/agent/Agent` | A2A server | agent on a subgroup, `@WeilSecured` |
| `a2a_basic/A2AAgent` | dev UI | `@WeilSecured` locally; calls the server above with `remoteAgent` |
| `WeilTravelFlights`, `WeilTravelHotels` | A2A servers | agent on `engg.weil`, `@WeilSecured` on each search |
| `WeilTravelPlanner` | dev UI | none of its own; calls both with `remoteTool` |
| `examples-web/.../WeilTravelPlannerWeb` | dev UI | as `WeilTravelPlanner` |
| `examples-web/.../spring_chat` | own runner behind `POST /api/chat` | none (memory only) |
| `examples-web/.../spring_chat_advanced` | own runner, streamed | `@WeilSecured` on Spring beans; `talkToHuman` open |

Commands for each: `ADK_EXAMPLES.md`. Hooks, records and writing an action:
`HOOKS.md`.
