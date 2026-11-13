# Ported ADK examples

Two sets live here: ports of the Python SDK's examples
(`python/examples/adk-ported`, `python/examples/mcp_secured_audit_starter.py`),
which behave as their Python originals, and four of the official adk-go
examples (below the table).

| Python example | Go port |
|---|---|
| `multi_agent/hello_world_ma` | `../guide/dice-prime` |
| `multi_agent/single_turn_sub_agent` | `multiagent/single_turn` |
| `multi_agent/sub_agents` | `multiagent/sub_agents` |
| `multi_agent/task_sub_agent` | `multiagent/task_sub_agent` |
| `multi_agent/three_layer_transfer` | `multiagent/three_layer_transfer` |
| `hitl/human_tool_confirmation` | `toolconfirmation` |
| `hitl/human_in_loop` | `hitl/human_in_loop` |
| `hitl/tool_confirmation` | `hitl/tool_confirmation` |
| `hitl/tool_human_in_the_loop_config` | `hitl/tool_human_in_the_loop_config` |
| `hitl/classifier_tool_gate` | `hitl/classifier_tool_gate` (needs the `sqlite3` CLI) |
| `hitl/request_input_tool` | `hitl/request_input_tool` |
| `adk_team/*` | `adk_team/*` (one shared identity, `WEIL_ADK_TEAM_AGENT_IDENTITY`) |
| `google-adk/weil-travel-a2a` | `../secure_travel`, `../guide/secure-travel` |
| `mcp_secured_audit_starter.py` | `go/weil_mcp/examples/secured_audit_starter` |

Every agent — sub-agents, workflow agents and A2A callers included — acts
under an identity of its own, chosen per agent by name (see
`internal/devnet`): `WEIL_IDENTITY_<AGENT>` (an Agent Registry
identity, `weil_...`), else `WEIL_WALLET_<AGENT>` (a wallet file), else
`~/.weilliptic/agents/<agent>.wc`, created on first use. Each example's
header lists its agents, their variables and the grants each needs; two
agents resolving to one wallet is an error. Keys such as `OPENAI_API_KEY`
come from the environment too; none is in the code. Where Python
calls `setup_secure_a2a` (Pensieve on by default), the root agent carries
`weiladk.PensieveTools()` and the runner `weiladk.WithSecureA2A`; where it
calls `to_secure_a2a`, the agent is served with `weiladk.ToSecureA2A` or
`NewExecutor` + `SecureA2A` on the same port.

## Upstream adk-go examples

Four of the official [adk-go examples](https://github.com/google/adk-go/tree/main/examples)
(v2.5.0), changed only as much as it takes to make them WeilChain agents.
Each file keeps its upstream license header; every change is marked with a
`// weil:` or `// model:` comment, so `diff` against the upstream file shows
exactly what porting costs.

| Example | Upstream | Shows | Lines changed |
|---|---|---|---|
| `quickstart` | `examples/quickstart` | one LLM agent | +41 −9 |
| `tools/multipletools` | `examples/tools/multipletools` | agents used as tools (`weiladk.NewAgentTool`), each with its own wallet and audited and traced with the root | +47 −13 |
| `workflowagents/sequential` | `examples/workflowagents/sequential` | custom (no-model) agents in a workflow, each with its own wallet | +33 −2 |
| `a2a` | `examples/a2a` | an agent served and called over secured A2A, traced on both sides | +61 −12 |

About half of each count is the `// model:` changes below, not WeilChain.

## The `// weil:` changes

1. Load each agent's identity: `devnet.Agent(ctx, name)` here (its
   `WEIL_IDENTITY_<NAME>`, `WEIL_WALLET_<NAME>`, or a local development wallet
   created on first use); `weiladk.FromIdentity` or `weiladk.FromWalletFile`
   elsewhere.
2. Attach it: `llmagent.New(cfg)` → `weiladk.NewLLMAgent(id, cfg)`, or
   `weiladk.Bind(agent, id)` for custom and workflow agents.
3. Make the plugin: `weiladk.NewPlugin(plugin.Config{}, weiladk.WithActions(weiladk.AuditAction(...)))`,
   and `weiladk.WithTracing(weiladk.TraceConfig{})` to record each run's tool
   calls as one trace (see Traces below).
4. Register it: `PluginConfig: runner.PluginConfig{Plugins: ...}` on
   `launcher.Config`.
5. A2A only: `adka2a.NewExecutor` → `weiladk.NewExecutor`, the handler wrapped
   in `weiladk.SecureA2A(..., weiladk.WithService("engg.weil"))`, and
   `remoteagent.NewA2A(cfg)` → `weiladk.NewA2A(id, cfg)`.

## The `// model:` changes

Not needed for WeilChain. The upstream examples run on Gemini, these on
Claude: `gemini.NewModel` → `claudemodel.NewModel`, and Gemini's built-in
`geminitool.GoogleSearch` (which only Gemini models can run) → the
`websearch::weil` applet through `weiladk.NewAppletTool`. With a Gemini key,
keep both as they were.

## Run

Against a local platform, with `API_KEY` (or `ANTHROPIC_API_KEY`) set, from
`go/weil_adk`:

```bash
export WEIL_SENTINEL_HOST=https://sentinel-local.weilliptic.ai   # default https://192.168.252.9
go run ./examples/ported/quickstart console
go run ./examples/ported/tools/multipletools console
go run ./examples/ported/workflowagents/sequential console
go run ./examples/ported/a2a console
```

In `multipletools`, `search_agent` and `poem_agent` run through
`agenttool`, which gives each its own runner without the outer plugins, so
the audit sees only `root_agent`: one run adds 4 records plus one per model
reply and two per tool call of `root_agent`, nothing from inside the
sub-agents. To audit an inner agent, use `SubAgents` instead of `agenttool`,
or give it callbacks of its own.

`console` is ADK's own launcher, as upstream; type a message and press Enter.
`CLAUDE_MODEL` picks the model (default `claude-opus-5-5`).

`a2a` serves only callers holding Execution or Management in `engg.weil`.
Its caller is the `travel-planner` wallet, which needs that grant (see
`secure_travel`). Without it the server refuses the call with 403, which
ADK's console launcher shows as an empty `Agent ->` reply: it does not print
error events (see GUIDE.md, step 7).

## Traces

Each run also records one trace on `weilchain::weil`: the user's message, then
every tool call with its arguments and the result the model received. When
the run ends, the trace is indexed and published on the `audits` topic of
`publisher::<org>` of the signing wallet (a wallet with no org keeps the trace
unpublished, as the Python SDK does). Every trace call is signed by the run's root agent's wallet, or the
fallback wallet if the root agent has none.

| Example | Trace per run |
|---|---|
| `quickstart` | one event per `web_search`, signed by `weather-agent` |
| `tools/multipletools` | one event per call to `search_agent` or `poem_agent`, signed by `search-agent`; the searches inside `search_agent` are not in it (`agenttool`, as above) |
| `workflowagents/sequential` | no events (no tools), signed by `desk` |
| `a2a` | two traces: the served agent's (`flight-agent`, its `web_search` calls), and the caller's (`travel-planner`, no events: handing the request to the remote agent is not a tool call). A refused call leaves only the caller's. |

Read them back with `get_similar_traces(query_str)` on `weilchain::weil`, or
from the `audits` topic through `consumer::weil`.
