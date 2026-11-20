# Securing agents over A2A (Java)

How the Java SDK decides who may call an agent and who may run its tools when
agents call each other over A2A, including what happens when a remote agent
calls another remote agent.

There are two checks, at two places:

| Check | Where | Wallet checked | Asks |
|---|---|---|---|
| **Gate check** (agent level) | the server of the agent being called (`WeilAgentServer`) | the **caller's** wallet, from the request's signature | may this caller use this agent at all? |
| **Tool check** (tool level) | inside the agent, just before a tool runs (`PermissionGate`) | the agent's **own** wallet (the one it runs as) | may this agent run this tool? |

The caller is checked once, at the door. Inside, each agent's tools are
checked against that agent's own wallet, not the caller's. This is how the Go
SDK works too.

Both checks ask the chain the same question: does this wallet hold
**Execution** or **Management** (or only **Management**, for admin checks) in a
subgroup, written `"<subgroup>.<org>"`, such as `engg.weil`?

---

## 1. Calling another agent

```java
Weil.remoteTool("flight_agent", "http://flights:8001", "Finds flights")   // as a tool
Weil.remoteAgent("flight_agent", "http://flights:8001", "Finds flights")  // as a sub-agent
```

Every A2A request is **signed by the calling agent's own wallet**
(`WEIL_AGENT_KEY`). The signature covers the A2A message (`params.message`),
and is bound to it by the body hash.

| Header | Holds |
|---|---|
| `X-Wallet-Address` | the caller's wallet |
| `X-Message` | the signed JSON: address, timestamp, body hash, nonce |
| `X-Signature` | the signature over `X-Message` |
| `X-Timestamp` | when it was signed |
| `X-Body-SHA256` | the SHA-256 of the A2A message |
| `X-Nonce` | a random value for this request |

Both also carry the caller's trace in the signed message (`weil_trace`), so
the called agent adds its work to the caller's trace; see `HOOKS.md`. It plays
no part in permissions.

---

## 2. Gate check: who may call an agent

```java
Weil.toSecureA2a(agent, 8001, "engg.weil");   // callers checked on engg.weil
Weil.toSecureA2a(agent, 8001);                // callers checked on WEIL_SERVICE
```

For every request, `WeilAgentServer` runs these steps in order, before the
model runs, so a refused caller costs nothing:

| # | Step | Refused with |
|---|---|---|
| 1 | **Signature.** It must verify, and match the message actually sent (body hash) | `401` |
| 2 | **Service gate.** The caller's wallet must hold Execution or Management in the service | `403` (`503` if the chain could not be asked) |
| 3 | **Your own rule, optional:** `.authorize(caller -> reason or null)` on the server builder | `403` |
| 4 | **Conversation owner.** A conversation (`contextId`) belongs to the wallet that started it, and another wallet cannot continue it or read its tasks | JSON-RPC error: "context … belongs to another wallet" |

After these steps, the server writes the verified caller into the run
(`WeilAgentServer.callerWallet(ctx)`). It overwrites anything the client sent
under that key.

**Verdicts.** Each service-gate result is published to the `security` topic of
`publisher::<org>`, signed by the agent that checked:

- `"<caller> passed @secured Execution check for agent <agent> in <service>"`
- `"<caller> failed @secured Execution and Management checks for agent <agent> in <service>"`

Refusals (401, 403, 503) are also written to the auditor applet.

**Rules.** The service can be a single subgroup or a rule over several, with the
same meaning as Rust's `#[secured_user]`:

```
"engg.weil"
"engg.weil | hr.weil"
"(engg.weil | research.weil) & !interns.weil"
```

They are evaluated **org level first**. If the wallet holds the org-level key
in every org the rule names, it passes without the rule being evaluated. So an
org-level `weil` wallet passes `hr.weil & !finance.weil` even if it is also in
finance. Otherwise each condition passes on the org-level key or on the
subgroup key.

---

## 3. Tool check: who may run a tool

```java
@WeilSecured("finance.weil")                        // Execution or Management
public static Map<String, Object> searchFlights(...) { ... }

@WeilSecured(value = "finance.weil", admin = true)  // Management only
public static Map<String, Object> cancelBooking(...) { ... }
```

| Way | Use it for |
|---|---|
| `@WeilSecured("svc.org")` on the tool's method | your own Java tools (`FunctionTool`) |
| `PermissionGate.on(...)` / `PermissionGate.admin(...)`, filtered by tool | tools with no Java method of yours: MCP tools, ADK's own tools |
| `Weil.agentTool(agent, "svc.org")` | a local agent used as a tool |

- **When:** just before the tool runs, on every call the model makes.
- **Wallet checked:** the **agent's own wallet**. That is the same whether it is
  chatting locally or serving an A2A request.
- **Refused:** the tool does not run, and the model gets `{"error": "Permission
  denied: wallet ... needs ... in <service>"}` as the tool's result. It can
  explain that to the user.
- **Fail-closed:** if the chain cannot be asked, the tool is refused.
- **Verdict:** published to `security`, as
  `"<agent wallet> passed @secured Execution check for tool <tool> in <service>"`
  or `failed ...`.
- **Rules:** the same rules as the gate, for example
  `@WeilSecured("hr.weil & !contractors.weil")`.

**Where `@WeilSecured` has no effect.** It is read only from the method behind
an ADK `FunctionTool` when the model calls it. It does nothing:

- on a method your own code calls directly;
- on MCP tools or model built-in tools (Google Search, code execution): use
  `PermissionGate.on`, or nothing can gate built-ins;
- on a hand-written `BaseTool` subclass;
- on an annotation placed on an interface or parent method;
- without the Weil plugin (`Weil.web`, `Weil.chat`, `Weil.runner`,
  `Weil.toSecureA2a` all add it).

A plain `AgentTool.create(agent)` runs its agent without the plugin. If that
agent holds `@WeilSecured` tools, the call is refused rather than run
unchecked. Use `Weil.agentTool(agent)` instead.

---

## 4. Planner → flights → pricing: every check

Three agents, each in its own process with its own wallet. The planner calls
flights with `remoteTool`, and flights calls pricing with `remoteTool`.

```
user ─▶ planner ──signed by planner──▶ flights ──signed by flights──▶ pricing
```

| # | Where | Check | Wallet checked | Needs |
|---|---|---|---|---|
| 1 | planner | planner's own tools | **planner** | the tool's service |
| 2 | flights server | signature | the **planner** (signer) | a valid signature over the message |
| 3 | flights server | gate (`AGENT_SERVICE` of flights) | **planner** | Execution or Management in flights' service |
| 4 | flights | `searchFlights` (`@WeilSecured("finance.weil")`) | **flights** | Execution or Management in `finance.weil` |
| 5 | pricing server | signature | **flights** (signer) | a valid signature |
| 6 | pricing server | gate (pricing's service) | **flights** | Execution or Management in pricing's service |
| 7 | pricing | pricing's own tools | **pricing** | each tool's service |

What follows from this:

- **Each hop only knows its direct caller.** Pricing sees flights, never the
  planner. Nothing about the planner is checked at pricing.
- **Rights are not inherited or narrowed along the chain.** If the planner may
  not use pricing but flights may, the planner still gets pricing's answers
  through flights. If flights may not use pricing, the call is refused even if
  the planner could.
- **Each agent needs the permissions its own tools ask for.** Flights' wallet
  needs `finance.weil` for `searchFlights`, whoever calls it.
- **Verdicts are attributed to the checker:** each gate verdict is signed by
  the agent that checked, and names the caller.

If you need "the person who started the chain must be allowed", that is not
supported today. It would need a signed "started by" token passed along the
chain; see the end of this file.

---

## 5. Setting up the wallets (travel example)

| Wallet | Needs |
|---|---|
| planner | Execution or Management in flights' and hotels' `AGENT_SERVICE` |
| flights | Execution or Management in `finance.weil` (`searchFlights`) |
| hotels | Execution or Management in `engg.weil` (`searchHotels`) |

If every agent runs from one shared env file, they all have the **same wallet**,
and every check is against that one wallet. Give each agent its own
`WEIL_AGENT_KEY` to see the checks above apply separately.

---

## 6. Known limitations

| Limitation | Effect | Planned fix |
|---|---|---|
| The age check reads the unsigned `X-Timestamp`, not the signed timestamp, and nonces are not remembered | A captured request can be replayed later as that wallet | Compare the signed timestamp with the header, and remember nonces for the age window |
| A remote agent's card is trusted as fetched | A wrong or hostile host receives your signed requests | Optional expected-wallet pin |
| `Weil.toSecureA2a(agent, port)` with `WEIL_SERVICE` unset or mistyped | The gate is off: any valid signature may call. The only sign is one startup line | Refuse to start, or warn loudly |
| Only the direct caller is known at each hop | No "started by" check across a chain | A signed origin token, if needed |

## See also

- `HOOKS.md`: every hook, the trace across A2A, and the audit records.
- `AGENT_GUIDE.md`: every way to run, serve and secure an agent.
- `src/main/java/com/weilliptic/weilwallet/SecuredRule.java`: the rule
  grammar and how rules are evaluated.
