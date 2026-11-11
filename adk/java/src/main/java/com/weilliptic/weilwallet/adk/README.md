# weilwallet.adk — WeilChain identity for Google ADK (Java)

Gives a Google ADK agent its own **wallet**, so its activity calls **applets**
on WeilChain as that agent — for auditing, for policy checks, or for anything
else you build.

This mirrors the Go SDK's `weil_adk` plugin design, which fits Java because
ADK Java exposes the same hook surface.

> **A2A.** Agents call each other over the A2A protocol, through ADK's
> `google-adk-a2a` module: `WeilA2A` signs each call with the caller's wallet,
> as the Python SDK does, and `WeilAgentServer` checks the signature and the
> caller's permission before the agent runs.

http://127.0.0.1:8080

## The idea

| Real world | Here |
|---|---|
| An employee | An ADK agent |
| Their ID badge and signature | A wallet (`WeilIdentity`) |
| Systems they use or report to | Applets on WeilChain |
| House rules | `WeilAction`s attached to the agent's lifecycle |
| The manager who enforces them | `WeilPlugin` |

Because every applet call is signed by the agent's own wallet, everything it
does on chain is attributable to it, and nobody else can do it in its name.

## Quick start

An ADK agent becomes a Weil agent with one call, plus an annotation on each
tool that needs a permission. Everything is read from the env file:
`WEIL_AGENT_KEY` (or `WEIL_WALLET_FILE`) and `SENTINEL_HOST` for the wallet,
and, for a served agent, `WEIL_SERVICE` for the subgroup a caller must hold
Execution or Management in.

```java
public static class Tools {
    @WeilSecured("engg.weil")                            // Execution or Management, as @secured_user
    public static Map<String, Object> searchFlights(...) { ... }

    @WeilSecured(value = "finance.weil", admin = true)   // Management only, as @secured_admin
    public static Map<String, Object> bookFlight(...) { ... }
}

LlmAgent agent = LlmAgent.builder()...tools(FunctionTool.create(Tools.class, "searchFlights")).build();

Weil.toSecureA2a(agent, 8001);              // serve over A2A: signatures, WEIL_SERVICE, audit, gates
Weil.toSecureA2a(agent, 8001, "engg.weil"); // the same, callers checked on this service instead
// or
Weil.chat(agent, args, "A question"); // run in the terminal; "-i" as the first arg: type as you go
// or
new InMemoryRunner(agent, "app", Weil.plugins());   // your own runner, or ADK's web UI
```

| Call | Gives |
|---|---|
| `Weil.toSecureA2a(agent, port)` / `Weil.secureA2aServer(agent)` | An A2A server, callers checked on `WEIL_SERVICE` (open when unset); `secureA2aServer` returns the builder for other options |
| `Weil.toSecureA2a(agent, port, service)` / `Weil.secureA2aServer(agent, service)` | The same, callers checked on the service given instead of `WEIL_SERVICE` |
| `Weil.chat(app, args, ...)` / `Weil.toSecureA2a(app, port[, service])` | The same for an ADK `App`: its settings (context caching, event compaction, resumability) and its own plugins are kept, the Weil plugin run first |
| `Weil.chat(agent, args, defaults...)` | A terminal run, flushed and closed at the end: the question in `args`, else `defaults`. With `-i` (or `--interactive`) first in `args`, each line typed at the `>` prompt is asked in one session, until `exit`, `quit` or Ctrl+D |
| `Weil.plugins(extra...)` / `Weil.runner(agent)` | The plugin for any runner: audit, the run trace, the `@WeilSecured` gates, then `extra` (e.g. a `PermissionGate` for MCP tools) |
| `Weil.remoteTool(name, url, description)` / `Weil.remoteAgent(...)` | Another Weil agent as a tool or a sub-agent, signed with this wallet |
| `Weil.withMemory(tools...)` / `Weil.memory()` | Graph memory over this wallet |
| `Weil.identity()` | The wallet, for anything built by hand |

Every way to run an agent (terminal, A2A server, your own runner, ADK's web
UI, YAML), to attach the plugin, and to secure agents and tools, with the
example that does each: **[AGENT_GUIDE.md](../../../../../../../AGENT_GUIDE.md)**.

The agent check (`WEIL_SERVICE`, Execution or Management) is done by `WeilAgentServer`; the
tool checks by `PermissionGate`, which reads `@WeilSecured`. Audit records and
traces go on an ordered background queue, so the agent waits only for the
gates; every record still queued is sent before the process exits, when a chat
ends or the process is stopped (one killed outright loses them). The exit
prints how they went, e.g. `[weil] records: 46 sent, 0 failed`. The wallet
starts loading in the background as soon as the plugin is made.

The same, built by hand, for several wallets or a plugin of another shape:

```java
WeilIdentity identity = WeilIdentity.fromApiKey(System.getenv("WEIL_AGENT_KEY"));

WeilPlugin plugin = WeilPlugin.builder(identity)
    .withActions(AuditAction.defaults())
    .flushAfterRun(60_000L)
    .build();

LlmAgent agent = LlmAgent.builder()
    .name("assistant")
    .model(new Claude("claude-sonnet-5-5", AnthropicOkHttpClient.fromEnv()))
    .tools(FunctionTool.create(Tools.class, "getBalance"))
    .build();

InMemoryRunner runner = new InMemoryRunner(agent, "my-app", List.of(plugin));
```

One plugin covers every agent the runner executes, including sub-agents.

## Local Sentinel certificate

A development Sentinel such as `sentinel-local.weilliptic.ai` serves a
certificate signed by the cluster's own CA, which Java does not trust out of
the box, so every call to it fails with `PKIX path building failed`. The SDK
verifies certificates by default, so make Java trust that one, once per
machine, in a truststore that also keeps Java's own authorities (the model APIs
need them):

```bash
openssl s_client -showcerts -connect sentinel-local.weilliptic.ai:443 -servername sentinel-local.weilliptic.ai </dev/null 2>/dev/null | awk '/BEGIN CERTIFICATE/,/END CERTIFICATE/' > ~/.weilliptic/sentinel-chain.pem
```

```bash
cp "$(java -XshowSettings:properties -version 2>&1 | awk -F'= ' '/java.home/{print $2}')/lib/security/cacerts" ~/.weilliptic/weil-truststore.p12
```

```bash
keytool -importcert -noprompt -alias weil-sentinel -file ~/.weilliptic/sentinel-chain.pem -keystore ~/.weilliptic/weil-truststore.p12 -storepass changeit
```

Then name it in each terminal before running an agent:

```bash
export JAVA_TOOL_OPTIONS="-Djavax.net.ssl.trustStore=$HOME/.weilliptic/weil-truststore.p12 -Djavax.net.ssl.trustStorePassword=changeit"
```

This imports the server's own certificate, which has to be done again when it
is renewed. Importing the cluster's CA (`local-cluster-ca`) instead, when you
have its certificate, keeps working across renewals. Check the names the
certificate is valid for with `openssl x509 -in ~/.weilliptic/sentinel-chain.pem -noout -ext subjectAltName`:
the Sentinel's host name must be among them, since Java checks that too.

### Turning verification off instead

For a quick run against a local Sentinel, `WEIL_TLS_VERIFY=0` (or `false`)
makes the SDK accept any certificate, as the Python SDK's `verify=False` does.
Set it in the env file or in front of the command; in code,
`TlsMode.trustAllCertificates()` does the same. It covers only the SDK's own
connections (the wallet, applet calls, audits, permission checks and signed
A2A calls), so the model APIs still check theirs, and a warning is printed
when it is turned off in code.

```bash
(set -a; source .env.hello; set +a; WEIL_TLS_VERIFY=0 mvn -q exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.helloworld.HelloWorldAgent)
```

Prefer the truststore. With verification off, anything between the agent and
Sentinel can pose as Sentinel, and the permission checks ask Sentinel whether
a wallet holds a purpose: a fake one can answer yes to every `@WeilSecured`
tool, YAML rule and server check. Use it only against a Sentinel on your own
machine or cluster.

## How it works

`Weil.chat` and `Weil.toSecureA2a` give the runner one `WeilPlugin`. ADK calls it at
every step of a run (user message, agent, model, tool, run end) for every agent
in the tree, and it runs the `WeilAction`s attached to that step, each building
an applet call signed by the agent's wallet:

| Action | Step | What it does |
|---|---|---|
| `AuditAction` | user message, agents, model replies, tools, run end | a record on the auditor applet, fire-and-forget |
| `TraceAction` | run start, model responses, before and after each tool, run end | one trace per run on `weilchain::weil`: the prompt, then each tool as a node with its arguments, result and parent, so tools called together branch; read back with `get_trace` when the run ends and published to `publisher::<org>`, topic `audits`. In `com.weilliptic.weilwallet`, beside `WeilClient` |
| `PermissionGate` | before each secured tool | checks the caller's wallet; refused, the tool does not run and the model gets the reason |

Non-blocking actions can never stop a step. By default a hand-built plugin
sends them before the hook returns (retried, and a failure reported);
`inBackground()`, which `Weil.plugin()` uses, or a queued `Dispatcher`, sends
them from a background queue instead, and `shutdown()` sends what is left.
Blocking ones wait and can refuse. **[HOOKS.md](../../../../../../../HOOKS.md) is the detailed
reference**: the building blocks (`WeilIdentity`, `Applet`, `Call`,
`WeilAction`), every hook and when ADK fires it, the audit record's fields,
blocking, vetoes and flushing, the hooks each example fires, and how to change
or add actions, such as a spending cap.

## Agent to agent

Agents talk over **A2A**, the open agent-to-agent protocol (version 0.3,
JSON-RPC transport): the same protocol the Python and Go SDKs serve through
ADK. A Java agent can call a Python one and the reverse. Each call is signed by
the calling agent's wallet and verified before the receiving agent's model runs,
so a caller that was not allowed to ask costs no model tokens.

**Serving side** — wrap an agent and listen:

```java
WeilAgentServer.builder(agent, identity)
    .port(8001)
    .requirePermission("engg.weil")   // callers need a purpose in this subgroup
    .plugins(List.of(weilPlugin))     // the served agent's own audit trail
    .build()
    .start();
```

It serves, on `http://127.0.0.1:<port>/` (or `publicUrl(...)`):

| | |
|---|---|
| `GET /.well-known/agent-card.json` | the A2A agent card (also at `/.well-known/agent.json`) |
| `POST /` | JSON-RPC `message/send`, `tasks/get`, `tasks/cancel` |

Three settings take their default from the environment, so an agent runs in a
container without code changes:

| Setting | Builder | Environment | Default |
|---|---|---|---|
| Port | `port(...)` | `PORT`, when code sets no port | 8001; the examples read `PORT` themselves, with their own port as the default |
| Address it listens on | `bindAddress(...)` | `WEIL_A2A_BIND` | `127.0.0.1`: only this machine can call. A container sets `0.0.0.0` |
| URL in the agent card | `publicUrl(...)` | `WEIL_A2A_PUBLIC_URL` | `http://<bind address>:<port>/`, or `127.0.0.1` when listening on all interfaces |

Clients send their calls to the card's URL, not to the URL they fetched the
card from, so an agent other hosts call needs its reachable URL in the card,
for example `http://flights:8001/` in Docker Compose.

The card advertises no streaming, so clients use `message/send`. A Weil
extension on the card (`https://weilliptic.ai/a2a/extensions/wallet-auth/v1`)
names the agent's wallet, the permission callers need, and the signing scheme.
The agent runs on ADK's own A2A executor, with the server's plugins.

Each request goes through three stages, and stops at the first failure:

| | | |
|---|---|---|
| 1 | signature and body binding | `401` |
| 2 | caller's wallet on the applet | `403` |
| 3 | run the agent | JSON-RPC result |

A refused caller is told, in one sentence its model can pass on, which wallet
was turned away and why: `wallet <address> is not authorised to perform
operations on <agent>: <reason>`. The same text is recorded as `message` on the
refusal's audit entry.

A failed *lookup* returns `503`, not `403`. "I could not ask" and "the answer
was no" are different problems, and collapsing them into one status makes a
wrong service name indistinguishable from a missing grant.

A conversation (`contextId`) belongs to the wallet that started it. Another
wallet cannot continue it (`message/send` is refused) or see its tasks
(`tasks/get` and `tasks/cancel` answer "task not found").

**Calling side** — `WeilA2A` builds ADK's `RemoteA2AAgent` with a signing
interceptor. Hold it as a **tool**:

```java
LlmAgent planner = LlmAgent.builder()
    .name("travel_planner")
    .model(model)
    .tools(
        WeilA2A.remoteTool("flight_agent", "http://127.0.0.1:8001", identity,
            "Searches for flights between cities."),
        WeilA2A.remoteTool("hotel_agent", "http://127.0.0.1:8002", identity,
            "Searches for hotels."))
    .build();
```

The URL is the remote agent's base URL (or its card's URL); the card is read
when the tool is built, so start the remote agent first. `WeilA2A.remoteAgent`
gives the `RemoteA2AAgent` itself, for use as a sub-agent, and
`WeilA2A.client(card, identity)` a signing a2a-java `Client` for direct calls.

A tool rather than a sub-agent, deliberately. ADK's sub-agent delegation hands
control over, which leaves a coordinator unable to collect two answers and merge
them — and invites a model to believe it has sent a request and is awaiting a
reply. A tool returns a value in the same turn, so the coordinator can call both
specialists and combine their answers.

Refusals come back as the tool's result, `{"error": "... HTTP 403: wallet ...
is not authorised ..."}`, rather than failing the turn, so the model reads the
reason and can explain it instead of retrying. The a2a transport reports only a
failed call's status; `WeilA2A` keeps the server's reply so the reason survives.

**What is signed.** The headers are the Python SDK's: `X-Wallet-Address`,
`X-Signature`, `X-Message`, `X-Timestamp`, plus `X-Body-SHA256` and `X-Nonce`.
The signed message binds `body_sha256`: the SHA-256 of the A2A message
(`params.message`; the params themselves for task calls), encoded as Python's
`json.dumps(..., separators=(",", ":"), sort_keys=True)` does: keys sorted at
every depth, no whitespace, non-ASCII escaped. `WeilAuth.canonicalJson` and
`WeilAuth.a2aSignedPayload` implement both halves, so a signature made by either
SDK checks out in the other. The envelope's JSON-RPC `id` is not covered: the
client transport assigns it after signing.

**Two addresses travel.** `X-Wallet-Address` carries the 64-hex
`sha256(public key)` — the only form a server can re-derive from a signature, so
it is what proves key ownership. The canonical sentinel-minted address carries a
pod-counter prefix that cannot be derived from a key at all, yet it is what
identity applets are keyed on, so it travels *inside the signed message* and is
accepted only when its tail matches the verified short form. See
`WeilAuth.canonicalAddress`.

**Not yet.** The Go SDK signs differently (a signed agent card, and a caller
proof inside the message), so Go and Java agents do not yet accept each other's
calls. Nonces are not remembered, so a captured request can be replayed within
the five-minute timestamp window.

**The caller reaches the agent.** The verified caller's wallet is put in the
A2A message's metadata (`weil.caller_wallet`), which ADK hands to the run, so
`@WeilSecured` and every `PermissionGate` check that wallet, not the served
agent's own (`WeilAgentServer.callerWallet`).

## Receipts on WeilStream

Three kinds, as the Python SDK writes them, so either SDK can read them.

**Permission verdicts.** Every permission check publishes a verdict (`passed`
or `failed`) to the `security` topic on the checked service's own
organisation's publisher, `publisher::<org>` (`finance.weil` goes to
`publisher::weil`), as Python does, naming the purpose checked and what it
guarded:

- `WeilAgentServer` on its callers:
  `"<wallet> passed @secured Execution check for agent <agent> in <service>"`
  (or `Management check`); refused,
  `"<wallet> failed @secured Execution and Management checks for agent ..."`
- `PermissionGate` on tools, naming the purpose the wallet passed on:
  `"<wallet> passed @secured Execution check for tool <tool> in <service>"`
  (or `Management check`). Refused, it names every purpose asked:
  `"<wallet> failed @secured Execution and Management checks for tool ..."`,
  or `"... failed @secured Management check ..."` for an admin gate.

Python words its verdict `"<wallet> passed @secured check for <service>"`;
Java's says which purpose and which agent or tool, since one wallet may pass
an agent's gate and fail a tool's. Verdicts are sent fire-and-forget, so the
check waits for the platform to accept them but not for the chain, and signed by the agent that made the check: the Python SDK
sends them unsigned in the checked wallet's name, Java signs them as the
checker, so the entry says who vouched. `publishPermissionChecks(false)` on the server turns it
off.

**Run traces.** Every run's trace, kept on `weilchain::weil` as the Python
SDK's tracer keeps it, is published to the organisation's publisher on topic
`audits` when the run ends, as `get_trace` returns it from the applet through
Sentinel: the prompt, then each
tool call as a node under the one it follows, so tools called together branch.
Each node is a `TraceEvent`, `{id, parent_id, event_type, content}`, with
`content` `{"args", "result", "tool"}` for a tool call. The applet also indexes it, so
`weilchain::weil.get_similar_traces(query_str, mode)` finds runs like a given
one. See HOOKS.md §4.2.

**Tool-call streams.** `StreamAudit` is Python's `stream_audit` for tools,
now opt-in (`Weil.plugin()` publishes the run trace instead):
before each tool runs, its arguments are published to the organisation's
publisher, `publisher::<org>`, on topic `audits` (or one you name), signed by
the acting agent. The payload is the arguments as Python's `json.dumps` writes
them, e.g. `{"origin": "Delhi", "destination": "Mumbai", "date": "15 November 2026"}`.

```java
WeilPlugin.builder(identity)
    .withActions(StreamAudit.defaults())                                // as Python: waits, a failed publish stops the tool
    .withActions(StreamAudit.builder("audits").nonBlocking().done())    // a failed publish never stops a tool
    .build();
StreamAudit.publish(identity, "audits", Map.of("sku", "abc"));          // outside an agent, e.g. in an HTTP handler
```

The travel examples stream every tool call, non-blocking. The topic must exist on the
chain and the wallet must belong to an organisation.

**Auditor receipts and topics.** On `WeilClient`:

```java
client.createTopic("my-topic", "what it holds", 1_000_000);   // publisher::weil
client.publish("my-topic", payload);                           // signed by this wallet
client.publish("publisher::acme", "audits", payload, true);    // another publisher, queued
ReceiptRef ref = client.persistReceipt(content);               // auditor validate_and_persist_receipt
Optional<String> back = client.getReceiptForCommit(commitHash);
```

Anyone can publish to a WeilStream topic, so a reader must check each entry's
publisher wallet before trusting it.

## Models

ADK Java ships two adapters, `Gemini` and `Claude` — and **no OpenAI one**, so
`gpt-4o` is not reachable without writing your own `BaseLlm`. `anthropic-java`
is already a compile dependency of `google-adk`, so Claude needs no extra
wiring:

```java
.model(new Claude("claude-sonnet-5-5", AnthropicOkHttpClient.fromEnv()))
```

`fromEnv()` reads `ANTHROPIC_API_KEY`. For Gemini, pass the model id as a
string instead and set `GOOGLE_API_KEY`.

None of this touches the Weil layer: the plugin wraps the agent, not the model,
so identity, auditing and gating behave the same whichever you pick.

## Examples

**Three agents** — a planner that calls a flights and a hotels specialist, each
in its own process with its own wallet. Three terminals:

```bash
mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelFlights
mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelHotels
mvn exec:java -Dexec.mainClass=com.weilliptic.weilwallet.examples.WeilTravelPlanner
```

Give each specialist its own `WEIL_AGENT_KEY`, or they sign as the same wallet
and the demo authorizes itself. Each specialist checks its callers on the
service its code names (`AGENT_SERVICE`, `engg.weil`), and each search tool on
its `@WeilSecured` service. `ADK_EXAMPLES.md` has the commands with their env
files.

**ADK's own samples**, ported with a wallet, in ADK's layout under `examples/`:
`helloworld`, `a2a_basic` with `a2a_server`, `mcpfilesystem`,
`city_time_weather` and `contextcaching`. Their YAML samples, with this repo's
own YAML agents, are in `examples/configagent/`, which has its own README.
adk-recipes' Java `financial-advisor` is in `examples/financialadvisor/`: a
coordinator with four specialist agents, each gated on its own service by `Weil.agentTool`,
and audited inside its own run. See its README.

## Layout

```
adk/
├── WeilIdentity.java   the agent's wallet; invoke, audit, auth headers
├── Applet.java         by address, service name, or systemic name
├── Call.java           one method invocation
├── WeilResult.java     unwraps the {"Ok"/"Err"} envelope
├── Hook.java           the 13 lifecycle points
├── WeilEvent.java      what an action sees when its hook fires
├── WeilAction.java     on this hook, call this applet
├── Mode.java           BLOCKING / NON_BLOCKING
├── FailurePolicy.java  FAIL_OPEN / FAIL_CLOSED
├── Dispatcher.java     delivers non-blocking calls: inline (default) or queued, with retries
├── WeilPlugin.java     plugs the actions into ADK's 13 callbacks
├── AuditAction.java    the auditor as a preset
├── PermissionGate.java Execution/Management-purpose gate for tools
├── Weil.java           one-call setup: wallet, plugin, gates, serve/chat, from the env
├── WeilSecured.java    @WeilSecured: a tool method's service and level
├── StreamAudit.java    tool-call arguments to a WeilStream topic (Python's stream_audit), opt-in
├── WeilA2A.java        A2A client: signed RemoteA2AAgent and tools
├── WeilAgentServer.java      serves an agent over A2A, signed callers only
├── WeilJsonRpcTransport.java registers the server's JSON-RPC transport with the a2a SDK
├── ClaudeModel.java    Anthropic Messages API as an ADK model, with prompt caching
├── Pensieve.java       graph memory on the Pensieve applet, scoped to the wallet
├── PensieveTools.java  the memory as 12 ADK tools; MemoryPrompt.java tells the model to use them, added by Weil.plugins()
├── WeilConfigAgents.java     loads YAML agents with a wallet (weil_memory, weil_remote_agent)
├── WeilConfigSecurity.java   reads a YAML file's weil: block
├── WeilMemoryToolset.java / WeilRemoteAgentToolset.java   those two YAML tool names
└── WeilYamlToJava.java writes the Java class a YAML agent tree stands for

../TraceAction.java     one trace per run on weilchain::weil, a graph of tool calls, published to "audits" (Python's tracer)
```
