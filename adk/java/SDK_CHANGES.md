# Java SDK: supporting files and why they changed

What this branch adds to the Java SDK beyond the agents themselves: every
supporting file that was added, changed or removed, what it does, and why. The
agents (the travel agents, the ports of ADK's samples, the financial advisor)
are left out; they are users of what is described here.

Paths are under `java/`; Java files are under
`src/main/java/com/weilliptic/weilwallet/` unless a path says otherwise.

**Two baselines.** `dev` already holds an earlier, squashed copy of this work
(commit `4dd41c80`, "java adk"). This branch was cut before it, so GitHub's
"Files changed" view, which compares with the branch point, shows the whole
ADK package as new; merging brings only what differs from that earlier copy.
§1 lists exactly that: each file new, changed or removed against `dev` as it
is now, and why. Sections 2 to 8 describe every file as the branch leaves it,
against `dev` before any ADK work. Merging `dev` into this branch before review
makes GitHub's view match §1.

- [1. Against `dev` today: what this merge changes](#1-against-dev-today-what-this-merge-changes)
- [2. The wallet SDK: changed files](#2-the-wallet-sdk-changed-files)
- [3. The ADK integration: new package `adk/`](#3-the-adk-integration-new-package-adk)
- [4. Supporting example files](#4-supporting-example-files)
- [5. Build, scripts and configuration](#5-build-scripts-and-configuration)
- [6. Documentation](#6-documentation)
- [7. Removed, and why](#7-removed-and-why)
- [8. Where Java and Python still differ](#8-where-java-and-python-still-differ)

---

## 1. Against `dev` today: what this merge changes

Measured against `dev`'s tip, which already has the earlier ADK copy.

### New files

| File | What it is | Why it is needed |
|---|---|---|
| `TraceAction.java` | One trace per agent run on `weilchain::weil`, kept as a graph: each tool call is a node `{id, parent_id, event_type, content}`, tools the model asks for in one response are siblings, a later tool follows the last one that finished, and a local `AgentTool` sub-agent is a `subtrace` node in its caller's trace. At the end of the run: `get_trace` (a Mermaid flowchart), `index_trace`, and the flowchart published to `publisher::<org>`, topic `audits` | The Java side of the Python SDK's tracer (`weil_ai/tracer.py`) and of the weilchain applet's graph trace API (`TraceEvent`, Mermaid `get_trace`): without it Java runs leave no trace, and with a flat list parallel tool calls look sequential. It sits in the core package, beside `WeilClient` and `TlsMode`, as Python's tracer is a module of its own beside its client rather than part of the ADK adapter; it uses the `adk` package only through public API |
| `adk/Weil.java` | The one-call entry point: `identity()`, `plugin()`/`plugins()`, `chat(...)`, `toSecureA2a(...)`/`secureA2aServer(...)`, `runner(...)`, `remoteTool`/`remoteAgent`, `memory()`/`withMemory(...)` (with the memory instructions added automatically), and the same for an ADK `App` | Every agent was wiring wallet, plugin, gates and runner by hand, each slightly differently; now an ADK agent becomes a Weil agent in one line. Replaces the helpers in the examples' `ExampleSupport` |
| `adk/WeilSecured.java` | `@WeilSecured("subgroup.org")`, or `admin = true`, on a tool method | Tool permissions written next to the tool, as Python's `@secured_user` / `@secured_admin` |
| `adk/SecuredAgentTool.java` | The tool `Weil.agentTool(agent, service)` returns: ADK's `AgentTool` with the plugins included, carrying the service a caller needs | The agent-level counterpart of `@WeilSecured`: the gate reads the service off the tool, so no gate is matched by name |
| `adk/WeilConfigAgents.java`, `WeilConfigSecurity.java`, `WeilMemoryToolset.java`, `WeilRemoteAgentToolset.java`, `WeilYamlToJava.java` | YAML agents: loading ADK Agent Config files with a wallet, the `weil:` permission block, the `weil_memory` and `weil_remote_agent` tool names, and YAML-to-Java generation (details in §3) | Agents written as YAML get the same identity, audit, trace and permissions as coded ones |
| `.env.example` | Every setting an agent reads, without keys | A template for the git-ignored `.env.*` files that replace the tracked ones (see Removed) |
| `AGENT_GUIDE.md`, `ADK_EXAMPLES.md`, `SDK_CHANGES.md` | Documentation (§6) | Every way to run, plug in and secure an agent; how to run each example; this file |

### Changed files

| File | What changed | Why |
|---|---|---|
| `WeilAuth.java` | Added `sortedPythonJson`: JSON as Python's `json.dumps(sort_keys=True)` writes it | A trace node's `content` must be byte-for-byte what the Python SDK writes, so traces from either SDK index and read alike |
| `WeilClient.java` | Removed `walletIsPermitted`. A permission lookup now asks the wallet's org-level key (its bare address) first, then its subgroup key (`<subgroup>_<address>`) | Callers now ask Execution and Management separately, to report which one passed. A wallet enrolled at org level holds its purpose in every subgroup, as Python checks it; before, it was refused everywhere but the org itself |
| `adk/Dispatcher.java` | An **inline** mode, now the default: a non-blocking call is made on the agent's thread before the hook returns. Every call is retried up to 3 times with backoff. `isInline()`, a retry count in `stats()` | With the old background queue, records still queued when a short process exits (a terminal chat) were lost, and a full queue dropped them. Inline is how Python writes audits; calls are fire-and-forget, so the wait is for the platform to accept a record, not for the chain. Retries stop one transient error from losing a record; a rare duplicate is visible on chain, a missing record is not. The queue is still there for those who want it |
| `adk/WeilPlugin.java` | Default dispatcher is inline. A tool vetoed before it runs now returns `{"error": reason}` as its result instead of failing the run | A failure from the before-tool callback aborted the whole run: the user, or an agent calling over A2A, got an empty answer. As a result, the model reads why and explains it |
| `adk/MemoryPrompt.java` | Added `plugin()`: appends the memory instructions to the system instruction of every agent holding the memory tools, before each model call, dated that day; skips agents without memory and instructions that already carry it. `Weil.plugins()` includes it | Every agent with memory had to append `MemoryPrompt.forAgent()` to its instruction by hand, and one that forgot got the tools without the rules, so it rarely used them. The date was also fixed when the agent was built, so a long-running server stamped memories with a stale one. The manual calls are gone from the examples |
| `adk/PensieveTools.java` | Added `isMemoryTool(tool)` | How the prompt plugin recognises an agent with memory |
| `adk/WeilEvent.java` | Added `toolCallId()`: the id the model gave the tool call | Pairs a tool's start and end in the trace, so parallel calls, even of the same tool, are recorded apart |
| `adk/PermissionGate.java` | Checks the **caller's** wallet (the one the A2A server verified), else the agent's own. Accepts Execution or Management; `admin(...)` takes Management only; `annotated(...)` gates every `@WeilSecured` tool, and refuses an `AgentTool` made without plugins whose agent holds `@WeilSecured` tools. Verdicts go to topic `security` on `publisher::<org of the service>`, name the purpose and the tool, and are published before the gate answers. A refusal reads `Permission denied: wallet ... needs ... permission in <svc>` | On a served agent the gate checked the agent's own wallet, so every caller passed whatever the agent itself held. Topic, publisher and levels now match Python's `@secured_user` / `@secured_admin`. Verdicts were fired on a background thread and could be lost; the message tells the model what to say. ADK runs an `AgentTool`'s agent without plugins by default, so its secured tools ran unchecked, silently |
| `adk/WeilAgentServer.java` | Callers need Execution or Management (was Execution only). The verified caller is stamped into the A2A request's metadata (`weil.caller_wallet`, overwriting anything the client sent) and read with `callerWallet(...)`. Can serve an ADK `App`. A blank `requirePermission` leaves the agent open and says so. Removed the pre-A2A `POST /invoke` and `/.well-known/weil-agent` endpoints and `Builder.path` | Same levels as Python's A2A gate, so an admin key need not enrol twice. Tool gates need to know who called, and only the server knows it for certain. `App` keeps context caching and compaction. An unset `WEIL_SERVICE` now turns the gate off instead of checking an empty service. The old endpoints served `RemoteWeilAgentTool`, which real A2A replaced |
| `adk/WeilA2A.java` | Sends the signed request itself and keeps the body of a failed response; honours `TlsMode`. Remote tools the model calls in one turn run at the same time, each on its own thread | a2a's JDK client turns a 401 or 403 into a fixed sentence and drops the body, which is where the server says which wallet was refused and why |
| `adk/ClaudeModel.java` | Prompt caching (on for Anthropic's own API, `CLAUDE_PROMPT_CACHE` overrides), cached tokens counted in usage, and functions read from every tool group | Cheaper, faster repeat calls; correct token counts; a tool group with no functions (Gemini's `google_search`) listed first used to hide every function after it |
| `adk/StreamAudit.java` | `publisherFor(identity)` is public | `TraceAction`, now outside `adk`, publishes to the same `publisher::<org>` |
| `adk/AuditAction.java`, `Mode.java`, `PensieveTools.java` | Comments only: what non-blocking means with the inline dispatcher; `RemoteWeilAgentTool` reference updated | Kept accurate after the changes above |
| `pom.xml` | Added `google-adk-langchain4j` and `langchain4j-open-ai`. Removed `spring-webmvc`, `javax.servlet-api` and the Spring exclusion | OpenAI and compatible servers as a model (`MODEL_PROVIDER=openai`). The Spring integration was removed (see below) |
| `examples-web/pom.xml` | Dropped the exclusion of the SDK's Spring | The SDK no longer brings Spring 5, which clashed with the dev UI's Spring Boot |
| `run.sh` | No longer exports `WEIL_TLS_VERIFY=0`; passes Java the truststore at `~/.weilliptic/weil-truststore.p12` (`WEIL_TRUSTSTORE` for another), and warns when there is none | Certificate checks stay on: with them off, anything between the agent and Sentinel could answer permission checks. Also fixes an empty-array expansion on macOS's bash 3.2 |
| `../.gitignore` | Ignores `java/.env`, `java/.env.*` (except `.env.example`) and `**/.commandcode/` | The env files hold agent keys |
| `HOOKS.md`, `adk/README.md` | Updated for the above: the graph trace, the dispatcher, the gates, A2A, TLS | Docs match the code |

### Removed files

| File | Why |
|---|---|
| `spring/` (4 files) and their libraries | Nothing used them, and Spring 5 clashed with the dev UI's Spring Boot |
| `agents/Verifiable`, `WeilAgentClient`, `WeilAgentHttpServer`, `WeilLocalCaller` | An early agent-to-agent design over plain HTTP, whose signature was not bound to the body; replaced by A2A (`WeilAgentServer`, `WeilA2A`) |
| `adk/RemoteWeilAgentTool.java` | The pre-A2A way to call an agent; replaced by `Weil.remoteTool` |
| `src/test/` (`WeilAuthRoundTripTest`, `AgentToAgentTest`, `WeilPluginTest`) | Written against the removed pre-A2A design; not yet replaced |
| Tracked env files: `.env.city`, `.dice`, `.flights`, `.hello`, `.hotels`, `.learning`, `.mcp`, `.multidice`, `.prime`, `.web`; `.commandcode/taste/taste.md` | They hold agent keys and must not be in git; now ignored, with `.env.example` as the template. The keys stay in `dev`'s history and should be rotated |

---

## 2. The wallet SDK: changed files

The core package, `com.weilliptic.weilwallet`, existed on `dev`. On `dev` it
could load a wallet from a local `wallet.wc` file, sign transactions with that
wallet's private keys, and call applets (`execute`, `audit`). The changes
below let it run AI agents: agents started from an Agent Registry API key,
agents that prove who they are to each other, and agents that check each
other's permissions on chain.

### `Account.java` (changed)

| | |
|---|---|
| Before | An account was always a full key pair: a private key and the Sentinel-minted address. The public key was worked out from the private key, and every account could sign. |
| Now | A second kind of account, built from a **public key and address only** (`Account(publicKey, address)`). The public key is stored up front for both kinds. Asking a public-key-only account to sign throws "account has no local secret key; use remote signing". |
| Why | An agent created in the Agent Registry gets an API key, not a private key: its private key stays on the Sentinel, which signs for it. The SDK still needs the account's address and public key (the public key goes in every transaction header), but cannot sign locally. Before this, such an account could not be represented. |
| Used by | `Wallet` in masked mode (below), so by every agent started with `WEIL_AGENT_KEY`. |

### `Wallet.java` (changed)

| | |
|---|---|
| Before | One way in, `Wallet.fromWalletFile(path)`, which reads `wallet.wc`, **re-derives** each derived account's private key from the master key (`xprv`), **reads** each external account's private key (`secret_key`), selects the active account and reads the organisation. A wallet without private keys could not be loaded. |
| Now | `Wallet.fromWalletJson(json, masked)`. With `masked = false` it is exactly the old behaviour, and `fromWalletFile` calls it that way. With `masked = true` it builds every account from its `public_key` and `account_address` alone, skipping `xprv` and `secret_key`. Account selection and organisation reading are shared and unchanged. |
| Why | Starting an agent from an API key returns the agent's wallet from the Sentinel **masked**: addresses and public keys, no private keys. The old loader required private keys. |

### `WeilAuth.java` (new)

| | |
|---|---|
| Before | Nothing: Java had no way for one agent to prove its identity to another over HTTP. |
| What it does | **Signs a request:** `buildHeaders(wallet, body)` produces `X-Wallet-Address`, `X-Signature`, `X-Message`, `X-Timestamp`, `X-Body-SHA256` and `X-Nonce`; the signed message carries a hash of the body, so the body cannot be swapped after signing. **Checks a request:** `verifyHeaders` (signature, timestamp window) and `verifyBody` (the body matches the signed hash). **Addresses:** a signature proves the 64-hex address (a hash of the public key), applets key on the 72-hex canonical one, and `canonicalAddress` accepts the canonical address from the signed message only when its tail matches the proven one. **JSON as Python writes it:** `canonicalJson` (`json.dumps(sort_keys=True, separators=(",", ":"))`, non-ASCII escaped), `pythonJson` (`json.dumps` defaults) and `sortedPythonJson` (`json.dumps(sort_keys=True)`, for trace nodes). `a2aSignedPayload` picks the part of an A2A request the signature covers. |
| Why | The A2A server's signature check (401) and permission check need a verified caller, and Java and Python agents must accept each other's signatures: each side re-hashes the body, so the JSON has to match byte for byte. The trace and the permission results use the same JSON writers so their payloads match Python's. |

### `TlsMode.java` (new)

| | |
|---|---|
| What it does | Whether the SDK's own HTTPS connections (wallet, applet calls, audits, permission checks, signed A2A calls) check certificates. On by default; `WEIL_TLS_VERIFY=0` (or `false`) or `TlsMode.trustAllCertificates()` turns it off, with a warning. The model APIs are not affected |
| Why | A development Sentinel's certificate is signed by the cluster's own CA, which Java does not trust. The recommended fix is a truststore (adk README, "Local Sentinel certificate"), which `run.sh` uses; this switch is the quick alternative for a Sentinel you run yourself, the Java counterpart of Python's `verify=False` |

### `TraceAction.java` (new)

See §1 and §3, "Actions the plugin runs". It is an ADK action, but lives in
the core package beside `WeilClient`, as Python's tracer does.

### `WeilClient.java` (changed)

On `dev`: load from a wallet file or a `Wallet`, `audit(log)`, `setAccount`,
`getReceiptForCommit`, account counts, `execute(...)` (always signed with the
local private key), and a plain HTTP client. All of that still works as it did.

| Added | What it does | Why |
|---|---|---|
| `fromApiKey(apiKey[, credentials, sentinelHost])` | Sends the API key to the Sentinel, loads the masked wallet it returns, and turns on remote signing | Agents run from `WEIL_AGENT_KEY` |
| Remote signing (`signMessage`, used inside `execute`) | When the account has no private key, the Sentinel signs each transaction | `execute` used to sign locally only, which fails for API-key agents |
| `walletHasExecutionPermission`, `walletHasManagementPermission` | Ask `identity::<org>.key_has_purpose`, first for the org-wide key (the bare address), then for the subgroup key (`<subgroup>_<address>`) | Every permission check: the agent check, `@WeilSecured`, `PermissionGate`, YAML rules |
| `resolveApplet`, `appletId` | An applet's address from its name (`publisher::weil`, `weilchain::weil`, `websearch::weil` …) | Calling applets by name, as Python does |
| `createTopic`, `publish` | Create a WeilStream topic; publish to one | The `audits` and `security` topics |
| `persistReceipt`, `auditArgs`, `unwrapResult`, `org`, `walletAddress`, `buildAuthHeaders` | Receipts; audit-record arguments with org and subgroup; reading `{"Ok"}`/`{"Err"}` results; the wallet's organisation; its address; A2A signing headers | Used by the audit, receipt and A2A code |
| HTTP client | Java's standard client, certificates checked unless `TlsMode` says otherwise | See `TlsMode` above |

---

## 3. The ADK integration: new package `adk/`

Everything in `adk/` is new against `dev` before the ADK work (§1 has what
changed since the copy already on `dev`). It plugs the wallet SDK into Google's Agent
Development Kit (ADK) for Java, version 1.11.0, through ADK's own extension
points: a `BasePlugin` for the steps of a run, `BaseLlm` for a model,
`BaseTool`/`BaseToolset` for tools, `ComponentRegistry` for YAML, and ADK's
A2A module for agent-to-agent calls. [`HOOKS.md`](HOOKS.md) is the detailed
reference for how the plugin, its actions and its records work.

### One-call setup

| File | What it does | Why |
|---|---|---|
| `Weil.java` | The entry point an agent uses: `Weil.identity()` (the wallet, from `WEIL_AGENT_KEY` or `WEIL_WALLET_FILE`), `Weil.plugin()` (audit trail, run trace, `@WeilSecured` gates), `Weil.chat(...)` (run at the terminal), `Weil.toSecureA2a(agent, port[, service])` (serve over A2A), `Weil.remoteTool` / `Weil.remoteAgent` (call another agent), `Weil.agentTool(agent[, service])` (a local agent as a tool, with the plugins, and gated on a service if one is given), `Weil.memory()` / `withMemory(...)`, and the same for an ADK `App`, keeping its settings (context caching, compaction, resumability) | So an ADK agent becomes a Weil agent in one line, with every agent set up the same way |
| `WeilSecured.java` | The annotation `@WeilSecured("subgroup.org")` (or `admin = true`) on a tool method: only a permitted wallet may run it | Java's equivalent of Python's `@secured_user` / `@secured_admin` |
| `WeilIdentity.java` | An agent's identity: a `WeilClient` plus `invoke(Call)` (sign and submit an applet call), `audit`, auth headers | One object the plugin, the server and the tools sign with |

### The plugin engine

| File | What it does | Why |
|---|---|---|
| `WeilPlugin.java` | Extends ADK's `BasePlugin`. ADK calls it at every step of every agent's run (user message, run, agent, model, tool, errors, end); it wraps each in a `WeilEvent` and runs the actions attached to that step. A refusal before a tool becomes the tool's result (`{"error": ...}`) so the model can explain it; elsewhere it ends the run. Flushes queued calls at the end of a run and on shutdown | The bridge between ADK's lifecycle and on-chain actions, covering every agent in a run |
| `WeilAction.java` | One action: *when* (hooks, optional filter), *what* (build a `Call` from the event), *how* (blocking or queued; fail open or closed) | What a user writes to put their own policy on chain |
| `WeilEvent.java` | What an action sees: the hook, the agent, the run id, the tool with its arguments, result and call id (`toolCallId()`), the model's reply, the user's message, the wallet | One shape for every step |
| `Hook.java` | The 13 points in a run an action can attach to | Names ADK's callbacks once |
| `Mode.java`, `FailurePolicy.java` | Blocking or not; failing open or closed | How an action behaves when the chain is slow or down |
| `Dispatcher.java` | The background queue behind queued actions: workers, queue size, overflow, timeouts, `flush`, `stats` | Audit writes take about two seconds; queuing keeps the agent from waiting on them |
| `Call.java`, `Applet.java`, `WeilResult.java` | One applet call (by name, address or systemic name), and its result with the `{"Ok"}`/`{"Err"}` envelope unwrapped | The shared vocabulary for applet calls |

### Actions the plugin runs

| File | What it does | Why |
|---|---|---|
| `AuditAction.java` | A record per step on the auditor applet, queued: who, what, when, with SHA-256 digests of tool arguments, results and model replies, or the content itself (`withContent()`, which `Weil.plugin()` uses) | A tamper-evident trail of what each agent did |
| `TraceAction.java` (in `com.weilliptic.weilwallet`, beside `WeilClient`) | One trace per run on `weilchain::weil`: `add_trace` with the user's prompt, then `build_trace` after each tool with a `TraceEvent` node `{id, parent_id, event_type, content}`, so tools called together are siblings and an `AgentTool` sub-agent is a `subtrace` node; then `get_trace` (a Mermaid flowchart), `index_trace`, and that flowchart published to `publisher::<org>` on topic `audits` | The Java side of Python's `weil_ai.tracer`: the whole run as one record, searchable with `get_similar_traces` |
| `PermissionGate.java` | Checks a wallet's purpose before a tool runs: `on(service)` (Execution or Management), `admin(service)` (Management only), `annotated(...)` (the tools carrying `@WeilSecured`, agent tools made with a service, and a plugin-less `AgentTool` hiding any). Publishes each result to `security` on `publisher::<org of the service>`, naming the purpose that passed. Shares `grantedBy` with the A2A server's agent check | Tool-level permissions, enforced from chain state, not the prompt |
| `StreamAudit.java` | Publishes a tool's arguments to a topic before it runs (Python's `stream_audit`); `publisherFor(identity)` names the `publisher::<org>` both it and `TraceAction` publish to | Kept as an opt-in extra; `Weil.plugin()` no longer adds it, since the run trace replaced the per-tool publish |

### Agent to agent (A2A)

| File | What it does | Why |
|---|---|---|
| `WeilAgentServer.java` | Serves an agent over A2A (JSON-RPC, protocol 0.3) with its agent card. Before an agent runs, every request is checked: signature and body binding (401), the caller's Execution or Management in the agent's service (403), an optional custom authorizer. A conversation belongs to the wallet that started it. The verified caller is passed into the run, so tool checks use the caller's wallet. Refusals are audited and each check's result published. Can serve an ADK `App`. The port, the address it listens on and the URL in its card default to `PORT`, `WEIL_A2A_BIND` (else `127.0.0.1`) and `WEIL_A2A_PUBLIC_URL` | Agents must know who is calling, and refuse callers without permission before spending model tokens. The env defaults let one agent run in a container or on a cloud platform without code changes: it must listen on all interfaces there, on the port the platform gives, and name a URL other hosts can reach, since clients call the card's URL |
| `WeilA2A.java` | The calling side: `remoteAgent(...)` (another agent as a sub-agent) and `remoteTool(...)` (as a tool), every message signed with the caller's wallet. A refusal reaches the calling model as `{"error": ...}` in the server's words | Calls to other agents that they can verify, with refusals the model can explain |
| `WeilJsonRpcTransport.java` and `src/main/resources/META-INF/services/io.a2a.server.TransportMetadata` | Registers the server's JSON-RPC transport with the a2a SDK | Without it the a2a SDK rejects the agent card at startup |

### Model

| File | What it does | Why |
|---|---|---|
| `ClaudeModel.java` | Extends ADK's `BaseLlm`: converts ADK's requests to the Anthropic Messages API and back. Skips reply blocks it does not use (such as `thinking`), keeps parallel tool calls, drops empty text blocks, cleans JSON schemas the API rejects, reads functions from every tool group, marks the fixed start of the prompt for Anthropic's prompt cache (on for Anthropic's own API; `CLAUDE_PROMPT_CACHE` overrides) and reports cached tokens, and records which endpoint served the call | ADK's own `Claude` model throws on `thinking` blocks, which current models send before tool calls, so any agent with tools failed on its first turn; it also disables parallel tool use and has no caching |
| `WeilAgentSetup.java` | Picks the model from `MODEL_PROVIDER`: Anthropic through `ClaudeModel`, or OpenAI (and compatible servers) through ADK's LangChain4j adapter, with a streaming model and a fix that gives ADK whole streamed turns. Moved here from `examples/` | An agent's code runs on either provider without change, and published with the SDK, so agents outside this repo (such as recipes) use it rather than copy it. Replaced `ExampleSupport`, whose wallet, plugin and chat helpers moved into `Weil` |

### Memory

| File | What it does | Why |
|---|---|---|
| `Pensieve.java` | Graph memory on the `pensieve::weil` applet, scoped to the wallet: entities, observations, relations; read, write, forget, search | Memory that lasts across sessions and belongs to the agent's wallet |
| `PensieveTools.java` | The memory as 12 ADK tools (`remember_batch`, `traverse`, `search_memory` …) | So the model can use memory; the same 12 as Python's |
| `MemoryPrompt.java` and `src/main/resources/com/weilliptic/weilwallet/adk/memory-prompt.txt` | The instructions that make the model read memory before answering and store only the facts and preferences the user states, as entities and relations: the prompt the icarus agent uses, with its `agent_id` line dropped (the Java tools fill it in) and the date added. `MemoryPrompt.plugin()`, part of `Weil.plugins()`, adds them to every agent holding the memory tools before each model call, dated that day; `forAgent()` gives the text for an instruction written by hand | Without them the model rarely uses the tools; built in, no agent has to remember to add them |

### YAML agents

| File | What it does | Why |
|---|---|---|
| `WeilConfigAgents.java` | Loads an agent tree from ADK's YAML format with a wallet: every `model:` name goes to the configured model, and the names `weil_memory` and `weil_remote_agent` resolve | Agents written as YAML get the same Weil features as coded ones |
| `WeilConfigSecurity.java` | Reads a file's optional `weil:` block: the agent's service (`agent: secured` / `open` / `{service: ...}`) and per-tool `user` / `admin` rules with their services | Permissions for YAML agents, beside ADK's own keys (ADK ignores the block) |
| `WeilMemoryToolset.java`, `WeilRemoteAgentToolset.java` | The two YAML tool names: graph memory, and another agent as a tool | Let a YAML file ask for memory or another agent in one line |
| `WeilYamlToJava.java` | Writes the Java class a YAML agent tree stands for, with its permissions | Move an agent from YAML to code without retyping it |

---

## 4. Supporting example files

Not agents, but shared by them.

| File | What it does | Why |
|---|---|---|
| `examples/WebSearchTools.java` | `web_search`: calls the `websearch::<org>` applet's `search` (or the applet `WEIL_SEARCH_APPLET` names), secured with `@WeilSecured("engg.weil")`; registered for YAML as `weil.web_search` | A web search any model can call and Weil can secure, unlike Gemini's built-in `google_search` |
| `examples/WeilYamlAgent.java` | Runs a YAML agent: chat at the terminal, or `--serve <port>` | The runner for every YAML agent |
| `examples/ConfigDiceTools.java`, `examples/ConfigTravelTools.java` | The dice and travel tools registered under the names YAML files use | ADK resolves YAML tool names through its registry |
| `examples/configagent/CustomDemoRegistry.java`, `CoreCallbacks.java`, `LifeAgent.java` | What ADK's YAML samples name in code: callbacks, a sub-agent, the few-shot `example_tool`, the dice and search tools | So Google's samples run as written |

---

## 5. Build, scripts and configuration

| File | Change | Why |
|---|---|---|
| `pom.xml` | Added Google ADK 1.11.0, ADK's A2A module, the a2a-java server and JSON-RPC transports, and ADK's LangChain4j adapter with LangChain4j's OpenAI client. Removed Spring and the servlet API | What the ADK integration and the OpenAI option need; Spring was only used by a package nothing used |
| `examples-web/pom.xml` (new) | A separate module for the web planner, on ADK's dev UI (Spring Boot) | It needs its own classpath |
| `run.sh` (new) | Starts `flights`, `hotels` or `web` with their env files, building and resolving the classpath only when needed. Passes Java the truststore at `~/.weilliptic/weil-truststore.p12` (`WEIL_TRUSTSTORE` for another) | One command per agent; the truststore lets Java trust the dev Sentinel's certificate with checking on |
| `.env.example` (new) | Every setting an agent reads, with no keys: wallet, Sentinel, service, Anthropic or OpenAI, prompt caching, the agents' URLs and ports, the search applet | A template for the real `.env.*` files |
| `../.gitignore` (root, changed) | Ignores `java/.env`, `java/.env.*` (except `.env.example`) and `.commandcode/` | The real env files hold keys and must not be committed |

---

## 6. Documentation

| File | What it is |
|---|---|
| [`AGENT_GUIDE.md`](AGENT_GUIDE.md) | Every way to run an agent (terminal, App, A2A server, own runner, ADK's web UI, YAML), to attach the Weil plugin, to secure agents and tools (`@WeilSecured`, `PermissionGate.on`/`admin` with filters, the server's service, YAML `weil:`), and to call other agents, with when to use each and the example that does it |
| [`ADK_EXAMPLES.md`](ADK_EXAMPLES.md) | Every ported agent with the command that runs it, and what to set up first (env files, truststore) |
| [`HOOKS.md`](HOOKS.md) | The detailed reference: the building blocks, every hook and when ADK fires it, the actions and what each records, blocking, refusals and flushing, the hooks each example fires, the A2A flow, and how to change or add actions |
| [`adk/README.md`](src/main/java/com/weilliptic/weilwallet/adk/README.md) | The overview: quick start with `Weil`, how it works, agent to agent, receipts and topics, models, examples, the package layout, and the "Local Sentinel certificate" setup |
| [`examples/configagent/README.md`](src/main/java/com/weilliptic/weilwallet/examples/configagent/README.md) | YAML agents: running them, the `weil:` permissions block, ADK's samples, `WeilYamlToJava` |
| `SDK_CHANGES.md` | This file |

---

## 7. Removed, and why

| Removed | Why |
|---|---|
| `spring/` (4 files) and their libraries | Nothing used them |
| `agents/WeilAgentClient`, `WeilAgentHttpServer`, `WeilLocalCaller`, `Verifiable`, and the `a2a_remote` and `AgentToAgentLocalExample` examples | An early agent-to-agent design over plain HTTP, replaced by real A2A (`WeilAgentServer`, `WeilA2A`). Its signature was not bound to the body, and its service format was out of date. `agents/WeilAgent.java`, which was on `dev`, stays |
| `adk/RemoteWeilAgentTool.java` and the server's `/invoke` endpoint | The pre-A2A way of calling an agent; replaced by `Weil.remoteTool` |
| `examples/ExampleSupport.java` | Renamed to `WeilAgentSetup`, now `adk/WeilAgentSetup.java`; its wallet, plugin and chat helpers moved into `Weil` |
| `WeilClient.walletIsPermitted` | Nothing called it once the checks were split by purpose |
| `src/test` | Written against the removed pre-A2A design; not yet replaced |
| Tracked `.env.*` files and `.commandcode/` | They held agent keys; now git-ignored, with `.env.example` as the template |

---

## 8. Where Java and Python still differ

Known differences, for anyone keeping the two SDKs in step.

| Area | Python | Java |
|---|---|---|
| Permission check | Asks `identity::<org>` for `key_management_addr`, then `key_has_purpose` on that key-manager applet | Asks `key_has_purpose` on `identity::<org>` directly |
| Audit records | Metadata only, except a `@secured` tool's arguments and response; fields `event`, `ts`, `caller_wallet` | `withContent()`: prompts, replies, tool arguments and results; fields `v`, `hook`, `time` (the Go SDK's format); no caller wallet |
| Permission results | Topic `security`, `"<wallet> passed @secured check for <service>"`, sent unsigned in the checked wallet's name | The same topic, `security`, naming the purpose and the agent or tool, signed by the checker |
| Memory tools | Added by default (`pensieve=True`) | Added when asked (`Weil.withMemory`, `weil_memory`), with instructions to use them |
| Trace indexing | The async finish (used by ADK) does not call `index_trace` | Always indexes |
| Trace branches | Siblings when tools run concurrently (asyncio tasks) | Siblings when the model asks for them in one response, whether ADK runs them in sequence or in parallel |
| A refused tool | Raises an error in ADK | Returns `{"error": ...}` as the tool's result |
| TLS | `verify=False` per client | A truststore (recommended, used by `run.sh`), or `WEIL_TLS_VERIFY=0` / `TlsMode.trustAllCertificates()` for the whole process |
