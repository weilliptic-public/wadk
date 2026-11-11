# Weil ADK reference: hooks, actions and records

The detailed reference behind [`adk/README.md`](src/main/java/com/weilliptic/weilwallet/adk/README.md),
which has the overview, quick start, agent-to-agent and models: the building
blocks, what each hook is, when ADK fires it, what our `WeilPlugin` does on it,
what it records, and which examples use it. Everything here is checked against
ADK **1.11.0** (the version in `pom.xml`) and this SDK's code; file references
are given so you can read the source.

- [1. How hooks work](#1-how-hooks-work)
- [2. One turn, hook by hook](#2-one-turn-hook-by-hook)
- [3. Hook reference](#3-hook-reference)
- [4. The actions we attach](#4-the-actions-we-attach)
- [5. Blocking, failures and vetoes](#5-blocking-failures-and-vetoes)
- [6. Hooks per example](#6-hooks-per-example)
- [7. A2A: two processes, two sets of hooks](#7-a2a-two-processes-two-sets-of-hooks)
- [8. Worked trace: HelloWorldAgent](#8-worked-trace-helloworldagent)
- [9. Changing what is hooked](#9-changing-what-is-hooked)
- [10. Where to read the code](#10-where-to-read-the-code)

---

## 1. How hooks work

```
 ADK Runner ──calls──► Plugin callbacks ──► WeilPlugin ──► WeilEvent(Hook) ──► WeilActions ──► applet calls
 (Google)              (Plugin.java)        (ours)         (who/what/when)     (audit, trace,    (signed by the
                                                                                gate ...)         agent's wallet)
```

1. **ADK defines the hook points.** `com.google.adk.plugins.Plugin` has one
   callback per point in a run (`beforeToolCallback`, `afterModelCallback` ...).
   Every callback has a do-nothing default.
2. **The runner calls every registered plugin at each point**, for every agent
   it runs: the root agent, its sub-agents, all of them. One plugin covers the
   whole agent tree in that process.
3. **`WeilPlugin` overrides all of them.** Each override wraps the callback in a
   `WeilEvent` tagged with a `Hook` (our enum), then runs every `WeilAction`
   registered for that hook, in registration order.
4. **A `WeilAction` turns the event into an applet call** (`Call`), signed by
   the agent's `WeilIdentity`, and sends it either without being able to stop
   the step (non-blocking) or waiting on its answer (blocking, and may veto).

The plugin is attached in one place: `new InMemoryRunner(agent, appName, plugins)`
in `Weil.chat`, or `WeilAgentServer.builder(...).plugins(...)` in `Weil.toSecureA2a`
for an agent served over A2A.

A hook with **no** action registered costs nothing: `WeilPlugin.has(hook)`
returns early before any work is done.

### The building blocks

| Type | What it is |
|---|---|
| `WeilIdentity` | The agent's wallet. `fromWalletFile(path)` for a local key, or `fromApiKey(key)` for an agent minted by the Agent Registry, which holds no private key locally and signs through Sentinel. `Weil.identity()` loads it from the env file |
| `Applet` | Which contract to talk to: `Applet.byId(contractId)`, `Applet.byName("counter::acme")`, or `Applet.systemic("auditor")` |
| `Call` | One method invocation: `Call.to(Applet.systemic("wusd"), "balance_for").args(Map.of("addr", identity.address())).build()` |
| `Hook` | A point in a run an action can attach to (§3) |
| `WeilAction` | What you write: *when* (hooks, optional filter), *what* (build a `Call` from the event), and *how* (blocking or not, failing open or closed) |
| `WeilPlugin` | Runs the actions on ADK's callbacks (above) |

---

## 2. One turn, hook by hook

A user asks one question; the model calls one tool, then answers.

```
user message arrives
 │
 ├─ USER_MESSAGE ........ onUserMessageCallback       ◄ audit: the prompt
 │                                                    ◄ trace: add_trace(trace_id, root = the prompt)
 ├─ BEFORE_RUN .......... beforeRunCallback
 │
 ├─ BEFORE_AGENT ........ beforeAgentCallback         ◄ audit   (agent "hello_world_agent")
 │   │
 │   ├─ BEFORE_MODEL .... beforeModelCallback
 │   │        ── LLM call #1 ──
 │   ├─ AFTER_MODEL ..... afterModelCallback          ◄ audit: reply + the tool call it asked for
 │   │   (EVENT)
 │   │
 │   ├─ BEFORE_TOOL ..... beforeToolCallback          ◄ audit: tool + args
 │   │                                                ◄ gate: @WeilSecured tools, MCP tools (refused → error result)
 │   │        ── tool runs ──            (or TOOL_ERROR if it throws)
 │   ├─ AFTER_TOOL ...... afterToolCallback           ◄ audit: tool + args + result
 │   │                                                ◄ trace: build_trace(trace_id, {tool, args, result})
 │   │   (EVENT)
 │   │
 │   ├─ BEFORE_MODEL
 │   │        ── LLM call #2 ──          (or MODEL_ERROR if it fails)
 │   ├─ AFTER_MODEL ..................................◄ audit: final reply
 │   │   (EVENT)
 │   │
 ├─ AFTER_AGENT ......... afterAgentCallback          ◄ audit
 │
 └─ AFTER_RUN ........... afterRunCallback            ◄ audit (then, with a queued dispatcher, wait ≤60 s for it to drain)
                          ◄ trace: get_trace, index_trace, publish the trace → publisher::<org> / "audits"
                          (or RUN_ERROR if the run failed; AFTER_RUN then does not fire)
```

`◄` marks where the examples' actions run. Every other hook still fires but has
no action attached.

With sub-agents, `BEFORE_AGENT` … `AFTER_AGENT` repeats for each agent that
takes a turn, and the handoff itself is a tool call (`transfer_to_agent`), so
it fires `BEFORE_TOOL` / `AFTER_TOOL` too.

---

## 3. Hook reference

All 13 values of [`Hook`](src/main/java/com/weilliptic/weilwallet/adk/Hook.java),
in lifecycle order. "Wire name" is what audit records store in `"hook"`.

| Hook (wire name) | ADK callback · where ADK calls it | Fires when | `WeilEvent` carries | Can a blocking action veto it? | Used by the examples |
|---|---|---|---|---|---|
| `USER_MESSAGE` (`user_message`) | `onUserMessageCallback` · `runner/Runner.java:658` | Once per user message, before anything runs | user message | Yes: run fails before it starts | **audit** |
| `BEFORE_RUN` (`before_run`) | `beforeRunCallback` · `runner/Runner.java:808` | Once per run, after the message is accepted | invocation | Yes: run fails | — |
| `BEFORE_AGENT` (`before_agent`) | `beforeAgentCallback` · `agents/BaseAgent.java:382` | Each time an agent starts its turn: root, sub-agent, or remote A2A agent | agent, its name | Yes: run fails | **audit** |
| `BEFORE_MODEL` (`before_model`) | `beforeModelCallback` · `flows/llmflows/BaseLlmFlow.java:319` | Before every LLM call (a turn with tools makes several) | model id, provider | Yes: run fails, no LLM call | — |
| `AFTER_MODEL` (`after_model`) | `afterModelCallback` · `flows/llmflows/BaseLlmFlow.java:399` | After every LLM call returns | model id, provider, the response | Yes: run fails | **audit** |
| `MODEL_ERROR` (`model_error`) | `onModelErrorCallback` · `flows/llmflows/BaseLlmFlow.java:360` | An LLM call threw (network, auth, rate limit) | model, the error | — | — |
| `BEFORE_TOOL` (`before_tool`) | `beforeToolCallback` · `flows/llmflows/Functions.java:631` | Before **every** tool call: your function tools, memory tools, MCP tools, A2A remote tools, `transfer_to_agent` | tool name, arguments | **Yes: the tool never runs** (see §5) | **audit**, **stream**, **gate** (MCP) |
| `AFTER_TOOL` (`after_tool`) | `afterToolCallback` · `flows/llmflows/Functions.java:705` | After a tool returns a result | tool, arguments, result | Yes: run fails (the tool already ran) | **audit** |
| `TOOL_ERROR` (`tool_error`) | `onToolErrorCallback` · `flows/llmflows/Functions.java` (`handleOnToolErrorCallback`) | A tool threw, **or** a `BEFORE_TOOL` action vetoed | tool, arguments, the error | — | — |
| `AFTER_AGENT` (`after_agent`) | `afterAgentCallback` · `agents/BaseAgent.java:394` | Each time an agent finishes its turn | agent | Yes: run fails | **audit** |
| `EVENT` (`event`) | `onEventCallback` · `runner/Runner.java:784` | Every event, before it is saved to the session | invocation | Yes: run fails | — |
| `AFTER_RUN` (`after_run`) | `afterRunCallback` · `runner/Runner.java:793` | Once, when the run **completes normally** | invocation | **Never.** A veto is only reported; the run is over | **audit**, then queue flush |
| `RUN_ERROR` (`run_error`) | `onRunErrorCallback` · `runner/Runner.java:821` | Once, when the run **fails** (including after a veto) | the error | **Never** | — |

ADK paths are under `adk-java/core/src/main/java/com/google/adk/` (your clone,
tag `v1.11.0`), or Cmd+click the callback in [`WeilPlugin.java`](src/main/java/com/weilliptic/weilwallet/adk/WeilPlugin.java).

Note that `MODEL_ERROR`, `TOOL_ERROR` and `RUN_ERROR` have **no action in the
examples**, so failures are not audited on chain. §9 shows how to add them.

---

## 4. The actions we attach

Every example gets its plugin from `Weil.plugin(extra...)`:

```java
WeilPlugin.builder(identity)
    .name("weil-audit")
    .inBackground()                                                             // §5: ordered background queue
    .withActions(AuditAction.withContent())                                     // 4.1
    .withActions(TraceAction.actions())                                          // 4.2
    .withActions(PermissionGate.annotated(identity, SENTINEL_HOST))             // 4.3, @WeilSecured tools
    .withActions(extra)                                                         // 4.3, e.g. MCP or YAML gates
    .onError((ev, action, err) -> System.out.println("  [weil] " + action + " on " + ev.hook() + ": " + err))
    .build();
```

Actions on the same hook run **in that order**: audit, then the trace, then
the `@WeilSecured` gate, then any extra.

### 4.1 `AuditAction.withContent()` — the audit trail

[`AuditAction.java`](src/main/java/com/weilliptic/weilwallet/adk/AuditAction.java)

| | |
|---|---|
| Hooks | `USER_MESSAGE`, `BEFORE_AGENT`, `AFTER_AGENT`, `AFTER_MODEL`, `BEFORE_TOOL`, `AFTER_TOOL`, `AFTER_RUN` (`AuditAction.DEFAULT_HOOKS`) |
| Applet | the systemic **auditor** applet, method `audit`, args `log` (JSON), `org`, `subgroup` |
| Mode | **non-blocking**, fire-and-forget: queued and sent in the background, in order (the platform accepts it; the chain executes it later); the agent does not wait, and it cannot veto |
| On failure | reported to `onError` (`[weil] audit on ...`); the agent carries on |

Each record (`AuditAction.record`) holds:

| Field | When present |
|---|---|
| `v`, `hook`, `time` | always |
| `agent`, `invocation`, `wallet` | when known (almost always) |
| `tool`, `args_sha256`, `result_sha256` | tool hooks |
| `llm`, `model`, `response_sha256` | `AFTER_MODEL` |
| `error` | when the event carries one |
| `prompt` | `USER_MESSAGE` — **content mode** |
| `args`, `result` | tool hooks — **content mode** |
| `response`, `tool_calls` | `AFTER_MODEL` — **content mode** |

`withContent()` is content mode: prompts, replies, tool arguments and results
are stored **in the clear**, each capped at 4000 characters
(`MAX_CONTENT_CHARS`) and marked when cut. Anyone who can read the chain can
read them. Use `AuditAction.defaults()` instead to store only the SHA-256
digests.

`llm` is who served the call: for `ClaudeModel`, the host of the Messages API
it talks to (`api.anthropic.com`, or `api.moonshot.ai` for Kimi through its
Anthropic-compatible endpoint), read from `ANTHROPIC_BASE_URL` as
`AnthropicOkHttpClient.fromEnv()` reads it; other models record their class
name. `model` is the model that answered when the reply names it, otherwise
the one the agent was configured with.

**Each hook costs a write.** The seven default hooks mean a single tool call
produces several records; narrow the hooks (§9) if that is more than you need.

### 4.2 `TraceAction` — one trace per run, published to `audits`

[`TraceAction.java`](src/main/java/com/weilliptic/weilwallet/TraceAction.java),
the Java side of the Python SDK's `weil_ai.tracer`. It sits beside
`WeilClient` and `TlsMode`, in `com.weilliptic.weilwallet`.

The trace is a graph: each node is the applet's `TraceEvent`,
`{id, parent_id, event_type, content}`, and `parent_id` is the node it
follows (null: the prompt). Tools the model asks for in one response are
siblings under the same parent, whether ADK runs them one after another or in
parallel (`RunConfig.toolExecutionMode`); a tool asked for later follows the
last tool that finished.

| Step | Hook | Applet call |
|---|---|---|
| Start | `USER_MESSAGE` | `weilchain::weil.add_trace(trace_id, root, [])`: a new UUID, the user's prompt as `root`. The applet records the signing wallet as `invoker` |
| Model response | `AFTER_MODEL` | none: the tools it asks for will follow the last tool that finished |
| Each tool | `BEFORE_TOOL`, then `AFTER_TOOL` | a node id and parent before it runs, keyed by ADK's function call id; after, `build_trace(trace_id, event)` with `event_type` `tool_call` and `content` `{"args", "result", "tool"}` as Python's `json.dumps(sort_keys=True)` writes it. The arguments carry `"_weil": {"framework": "adk", "tool_call_id": ...}`. A refused tool's refusal is its result |
| Sub-agent behind an `AgentTool` | its run's `USER_MESSAGE` | `build_trace` with a `subtrace` node, `{"root": request}`, under the tool call that started it; its tools follow that node in the caller's trace, and its run publishes nothing of its own |
| End | `AFTER_RUN` (or `RUN_ERROR`) | `index_trace` for similarity search, then `get_trace`, which reads the trace back from the applet through Sentinel, and what it returns published to `publisher::<org>` on topic `audits`; both on the background worker, so the agent does not wait |

| | |
|---|---|
| Mode | **non-blocking**: each step builds its call and the plugin's dispatcher sends it fire-and-forget; on `Weil.plugin()`'s ordered queue they go out one at a time in the order made, so a trace is started before its nodes, and the agent waits for none of them, not even `index_trace`, a query Sentinel holds until done. A failure is reported (`[weil] trace:... on ...`) and the agent carries on |
| Scope | one trace per run, keyed by ADK's invocation id: sub-agents in the run, and `AgentTool` sub-agents handed the plugin, add to it; concurrent runs on a served agent each get their own |
| A failed start | nothing more is written for that run, since `build_trace` would otherwise create a trace with no prompt |
| Needs | the wallet must belong to an organisation, and the `audits` topic must exist on its publisher |

Each run's trace replaces the per-tool stream earlier versions published to
`audits`. `StreamAudit` (Python's `stream_audit`: each tool's arguments to a
topic, before it runs) is still there for those who want it, but no longer in
`Weil.plugin()`.

### 4.3 `PermissionGate` — on-chain permission for tools

Every way to gate an agent or a tool, and when to use each: [AGENT_GUIDE.md §3](AGENT_GUIDE.md#3-ways-to-secure-an-agent).

[`PermissionGate.java`](src/main/java/com/weilliptic/weilwallet/adk/PermissionGate.java)

Added by `Weil.plugin()` for every tool method carrying `@WeilSecured`, and
passed explicitly for tools with no method to annotate: in
`McpFilesystemAgent`, `PermissionGate.on("engg.weil", ...)` filtered to every
MCP tool (`ev.tool() instanceof AbstractMcpTool`).

| | |
|---|---|
| Hook | `BEFORE_TOOL`, filtered to the tools it secures |
| Check | the caller's wallet (the agent's own in a chat) holds Execution, else Management, in the service (`admin`: Management only). A read-only query, not a transaction |
| Verdict | published fire-and-forget to topic `security`: `"<wallet> passed @secured Execution check for tool <tool> in <svc>"` (or `Management check`, whichever it holds); refused, `"<wallet> failed @secured Execution and Management checks for tool <tool> in <svc>"` |
| Mode | **blocking, fail-closed**: the tool waits for the check; if the check cannot be made, the tool is refused |
| Refusal | the tool does not run, and the model gets `{"error": "Permission denied: wallet ... does not have permission to run <tool>. It needs ... permission in <svc>."}` as its result — see §5 |

Memory tools are never gated.

Because the audit runs first on `BEFORE_TOOL`, a refused call still leaves its
`before_tool` audit record, and the refusal, as the tool's result, is in the
run's trace: the attempt is on record even though the tool never ran.

---

## 5. Blocking, failures and vetoes

Every action has a **mode** ([`Mode`](src/main/java/com/weilliptic/weilwallet/adk/Mode.java))
and a **failure policy** ([`FailurePolicy`](src/main/java/com/weilliptic/weilwallet/adk/FailurePolicy.java)).

| | Non-blocking | Blocking |
|---|---|---|
| Hook waits for the applet? | Yes with the default inline `Dispatcher`; no with a queued one, as `Weil.plugin()` uses | Yes |
| Can it stop the step? | **Never** | Yes, by throwing `WeilAction.VetoException` |
| Infrastructure failure (chain down, bad args) | reported to `onError`, step continues | `FAIL_OPEN`: reported, step continues · `FAIL_CLOSED`: becomes a veto |
| Used for | audit and the trace (in the examples) | permission gate, spend caps, approvals |

A `VetoException` thrown on purpose (e.g. by the gate, or from an `onResult`
handler) **always** vetoes, whatever the failure policy says. On `AFTER_RUN`
and `RUN_ERROR` nothing can veto: the error is only reported.

### What a veto does — verified on ADK 1.11.0

On `BEFORE_TOOL`, `WeilPlugin` turns a veto into the tool's result:

1. **The tool does not run.** ✔
2. ADK takes `{"error": "<veto reason>"}`, returned from the before-tool
   callback, as the tool's response; `AFTER_TOOL` still fires and records it.
3. **The model sees the refusal** and explains it to the user, or to the
   agent that called over A2A, and the run goes on.

On any other hook a veto is returned as `Maybe.error(veto)`, which ADK
re-throws, so **the whole run fails**: `RUN_ERROR` fires, `AFTER_RUN` does
not, and `runner.runAsync(...)` throws `VetoException` to the caller. No
action shipped here vetoes outside `BEFORE_TOOL`.

(A remote A2A agent that refuses a call is different: `WeilA2A.remoteTool`
catches the HTTP 403 and returns `{"error": "..."}`, so the model does see that
one. See §7.)

### Delivering non-blocking calls

Non-blocking calls go through the plugin's `Dispatcher`. Each call is tried
up to 3 times (0.5 s, then 1 s apart) before `onError` hears it failed. A
retry after a lost reply can record an entry twice; a duplicate shows on
chain, a missing entry does not.

`Weil.plugin()` uses an **ordered background queue** (`inBackground()`, which
is `Dispatcher.ordered()`): one worker sends the calls one at a time in the
order they were made, so the agent waits on none of them, only on blocking
gates. The queue holds 10,000 calls; when full, the hook sends its call itself
rather than drop it. Records land a little after the step they describe, and
nothing is flushed after each run:

- `plugin.shutdown()`: send every call still queued, however long it takes
  (each attempt times out with its HTTP request, so it cannot hang), then stop
  the dispatcher. `Weil.shutdown()` calls it for every plugin `Weil` made, when
  a chat ends and when the process stops (Ctrl+C, a container stop), and prints
  `[weil] sending N queued record(s)...` and `[weil] records: 46 sent, 0 failed`.
- A process killed outright (`kill -9`, a crash) loses what was still queued.
  Docker kills a container 10 s after asking it to stop; give it a longer stop
  timeout if many records may be queued.
- `WeilPlugin` is not `AutoCloseable`: ADK's `Plugin.close()` returns a
  `Completable`, which collides with `AutoCloseable.close()`, so it cannot go
  in try-with-resources. Call `shutdown()`.

A hand-built plugin is **inline** by default (`Dispatcher.inline()`), as the
Python SDK writes its audits: the call is made before the hook returns, one
round-trip per record on the agent's thread. Pass a queued one,
`.dispatcher(new Dispatcher(...))`, for your own worker count, queue size and
overflow policy, with `flush`, `close` and `stats`; `flushAfterRun(ms)` makes
each run's end wait for its queue to drain.

---

## 6. Hooks per example

All examples use audit (4.1) and the trace (4.2). `BEFORE_TOOL` / `AFTER_TOOL`
fire for every tool in the "Tools" column, and each tool call is one event in
the run's trace.

| Example | Env file | Agents (each fires `BEFORE_AGENT` / `AFTER_AGENT`) | Tools (each fires `BEFORE_TOOL` / `AFTER_TOOL`) | Extra |
|---|---|---|---|---|
| `HelloWorldAgent` | `.env.hello` | `hello_world_agent` | `rollDie`, `checkPrime`, 12 memory tools | — |
| `CityTimeWeather` | `.env.city` | `multi_tool_agent` | `getCurrentTime`, `getWeather`, 12 memory tools | — |
| `McpFilesystemAgent` | `.env.mcp` | `filesystem_agent` | 14 MCP filesystem tools, 12 memory tools | **gate** on every MCP tool, on `engg.weil` |
| `A2AAgent` | `.env.dice` | `root_agent`, `roll_agent`, `prime_agent` (remote) | root: 12 memory tools, `transfer_to_agent` · roll: `rollDie` | see §7 |
| `Agent (a2a_server)` | `.env.prime` | `check_prime_agent` | `checkPrime` | server-side caller check, see §7 |

The memory tools are ordinary tools, so every memory read and write is itself
audited and traced.

---

## 7. A2A: two processes, two sets of hooks

`A2AAgent` and `Agent (a2a_server)` run in separate JVMs, each with its own
wallet and its own `WeilPlugin`. Each plugin sees only its own process.

```
 A2AAgent      (wallet D, .env.dice)                       Agent (a2a_server)  (wallet P, .env.prime)
 ───────────────────────────────────                       ───────────────────────────────────────
 BEFORE_AGENT root_agent
 BEFORE_TOOL  transfer_to_agent → prime_agent
 BEFORE_AGENT prime_agent (RemoteA2AAgent)
     │  A2A JSON-RPC, signed by wallet D  ───────────────►  verify signature (not a hook)
     │                                                       requirePermission(AGENT_SERVICE) on D (not a hook)
     │                                                         refused → HTTP 403, refusal audited
     │                                                       USER_MESSAGE, BEFORE_RUN
     │                                                       BEFORE_AGENT check_prime_agent
     │                                                       BEFORE/AFTER_MODEL, BEFORE/AFTER_TOOL checkPrime
     │                                                       AFTER_AGENT, AFTER_RUN       ← audited under wallet P
     │  answer  ◄────────────────────────────────────────
 AFTER_AGENT  prime_agent
 AFTER_AGENT  root_agent, AFTER_RUN                         ← audited under wallet D
```

The checks on the server side happen in `WeilAgentServer` **before** the
served agent's runner starts, so they are not ADK hooks:

| Server check | Builder option | Default |
|---|---|---|
| Caller's wallet signature over the A2A message | always on | — |
| Caller's wallet holds Execution or Management purpose on the service | `.requirePermission(service)`: `Weil.toSecureA2a(agent, port, service)`, or `WEIL_SERVICE` for `Weil.toSecureA2a(agent, port)` | off when neither gives one |
| Verdict published to `security` | `.publishPermissionChecks(...)` | on |
| Refused request recorded on the auditor applet | `.auditRefusals(...)` | on |

A refusal reaches the caller as HTTP 403. Held as a **tool**
(`WeilA2A.remoteTool`, as `WeilTravelPlanner` does), it becomes
`{"error": "HTTP 403: wallet ... is not authorised ..."}` and the model can
explain it. Held as a **sub-agent** (`WeilA2A.remoteAgent`, as `A2AAgent`
does), the failure surfaces from the remote agent instead.

---

## 8. Worked trace: HelloWorldAgent

Question: *"Roll a 6-sided die and a 10-sided die, and tell me which results
are prime."* Assume the model first reads memory, then rolls both dice in
parallel, checks primes, writes memory, and answers.

| # | Hook | Audit record (`hook` / key fields) | Trace on `weilchain::weil` |
|---|---|---|---|
| 1 | `USER_MESSAGE` | `user_message`, `prompt` | `add_trace`, root = the prompt |
| 2 | `BEFORE_AGENT` | `before_agent`, `agent: hello_world_agent` | — |
| 3 | `AFTER_MODEL` | `after_model`, `tool_calls: [memory read]` | — |
| 4 | `BEFORE_TOOL` | `before_tool`, `tool: <memory read>`, `args` | — |
| 5 | `AFTER_TOOL` | `after_tool`, `result` | `build_trace`: the read |
| 6 | `AFTER_MODEL` | `after_model`, `tool_calls: [rollDie, rollDie]` | — |
| 7–8 | `BEFORE_TOOL` ×2 | `tool: rollDie`, `args: {"sides":6}` / `{"sides":10}` | — |
| 9–10 | `AFTER_TOOL` ×2 | `result: {"result":4,...}` … | `build_trace` ×2: each roll with its result |
| 11 | `AFTER_MODEL` | `tool_calls: [checkPrime]` | — |
| 12–13 | `BEFORE_TOOL` / `AFTER_TOOL` | `tool: checkPrime`, `args: {"nums":[4,7]}` | `build_trace`: the check |
| 14… | memory writes | one `before_tool` / `after_tool` pair each | `build_trace` each |
| … | `AFTER_MODEL` | `response: "You rolled 4 and 7 … 7 is prime"` | — |
| … | `AFTER_AGENT` | `after_agent` | — |
| last | `AFTER_RUN` | `after_run` | `get_trace`, `index_trace`; the whole trace published to `audits` |

Every record carries `wallet` = the agent's address and the same
`invocation` id, so one question's records can be grouped. The exact number of
model and memory calls depends on the model.

---

## 9. Changing what is hooked

All of this goes in `Weil.plugin`, or in a plugin built directly with
`WeilPlugin.builder(identity)`.

| To … | Use |
|---|---|
| Store digests only, not content | `AuditAction.defaults()` instead of `withContent()` |
| Audit failures too | **replace** `withContent()` with `AuditAction.builder(Applet.auditor(), true).on(Hook.MODEL_ERROR, Hook.TOOL_ERROR, Hook.RUN_ERROR).done()` (the defaults plus the three error hooks) |
| Audit more hooks | `AuditAction.on(Hook.BEFORE_MODEL, Hook.EVENT)` — the defaults **plus** these |
| Audit fewer hooks | not possible through `AuditAction`: its builder starts from `DEFAULT_HOOKS` and `.on(...)` only adds. Build your own `WeilAction` on the hooks you want, with `AuditAction.record(ev, true)` as the log |
| Refuse to run any step that cannot be audited | `AuditAction.builder().blocking().failClosed().done()` |
| Also publish each tool's arguments, as Python's `stream_audit` (wait, fail closed) | `StreamAudit.on("audits")` as an extra action |
| Stream only some tools | `StreamAudit.builder("audits").when(ev -> "rollDie".equals(ev.toolName())).nonBlocking().done()` |
| Gate every tool on a subgroup | `PermissionGate.on("engg.weil", identity)` |
| Gate some tools | `PermissionGate.on(svc, identity, sentinelHost, ev -> Set.of(...).contains(ev.toolName()))` |
| Write your own check | a blocking `WeilAction` that vetoes, as below |

A spending cap, refusing any payment over a quarter of the treasury:

```java
WeilAction.named("spend-cap")
    .on(Hook.BEFORE_TOOL)
    .when(ev -> "payInvoice".equals(ev.toolName()))
    .blocking()
    .failClosed()
    .build(ev -> Call.to(ledger, "balance_for")
                     .args(Map.of("addr", ev.identity().address()))
                     .build())
    .onResult((ev, result) -> {
        long balance = result.decode().asLong();
        long amount  = ((Number) ev.toolArgs().get("amount")).longValue();
        if (amount > balance / 4) {
            throw new WeilAction.VetoException("POLICY: exceeds 25% of treasury");
        }
    })
    .done();
```

A `VetoException` from `onResult` vetoes whatever the failure policy says:
`failOpen`/`failClosed` govern only infrastructure failures (the chain
unreachable, arguments that will not encode), never a deliberate refusal.

---

## 10. Where to read the code

| What | File |
|---|---|
| ADK's hook interface (all callbacks, with Javadoc) | `adk-java/core/src/main/java/com/google/adk/plugins/Plugin.java` |
| ADK's base class we extend | `adk-java/core/src/main/java/com/google/adk/plugins/BasePlugin.java` |
| How ADK runs several plugins (first non-empty result wins) | `adk-java/core/src/main/java/com/google/adk/plugins/PluginManager.java` |
| Where tool hooks fire, and how a veto becomes an error | `adk-java/core/src/main/java/com/google/adk/flows/llmflows/Functions.java` (≈ lines 480–720) |
| Our plugin: callback → `WeilEvent` → actions | [`WeilPlugin.java`](src/main/java/com/weilliptic/weilwallet/adk/WeilPlugin.java) (callbacks ≈ 73–171, firing ≈ 225–360) |
| The hook list | [`Hook.java`](src/main/java/com/weilliptic/weilwallet/adk/Hook.java) |
| What an event carries | [`WeilEvent.java`](src/main/java/com/weilliptic/weilwallet/adk/WeilEvent.java) |
| Building actions: hooks, filter, mode, policy | [`WeilAction.java`](src/main/java/com/weilliptic/weilwallet/adk/WeilAction.java) |
| Audit, trace, stream, gate | [`AuditAction.java`](src/main/java/com/weilliptic/weilwallet/adk/AuditAction.java), [`TraceAction.java`](src/main/java/com/weilliptic/weilwallet/TraceAction.java), [`StreamAudit.java`](src/main/java/com/weilliptic/weilwallet/adk/StreamAudit.java), [`PermissionGate.java`](src/main/java/com/weilliptic/weilwallet/adk/PermissionGate.java) |
| The plugin every agent shares | [`Weil.java`](src/main/java/com/weilliptic/weilwallet/adk/Weil.java) (`plugin`) |
| Server-side A2A checks | [`WeilAgentServer.java`](src/main/java/com/weilliptic/weilwallet/adk/WeilAgentServer.java) |
