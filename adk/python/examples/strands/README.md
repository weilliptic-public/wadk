# Strands Secure Hooks

The Strands Agents equivalent of the [Google ADK](../google-adk) and
[LangChain](../langchain) secure setups.

Use normal Strands tools and agents, but create the agent through Weil's Strands
helper inside a Weil identity:

```python
@tool
@weil_ai.secured_user("engg.acme")
def multiply(a: int, b: int) -> int:
    return a * b


@weil_ai.agent(identity="weil_...")
def create_math_agent():
    return weil_ai.strands.create_agent(model=model, tools=[add, multiply])
```

Put `@tool` outside `@secured_user`.

`weil_ai.strands.create_agent(...)` calls Strands' own `Agent(...)` with the
same arguments, adds the Pensieve memory tools, and installs
`WeilStrandsHooks`, a Strands `HookProvider`:

| Strands event | What Weil does |
|---|---|
| `BeforeInvocationEvent` | starts the run trace (`add_trace`, the prompt as root), audits `strands.agent.before` |
| `BeforeToolCallEvent` | for a `@secured` tool: audits `strands.tool.before` with its arguments, and sets the agent's wallet so the tool checks that wallet's permission |
| `AfterToolCallEvent` | adds the call to the trace (`build_trace`); for a `@secured` tool: restores the wallet context and audits `strands.tool.after` with its response |
| `AfterInvocationEvent` | audits `strands.agent.after`, then publishes the trace |

Weil's hooks run first on the before events and last on the after events, so
the trace and audit records hold what the model actually received after any
other hooks changed it. A refused `@secured` call reaches the model as an error
result, and is recorded in the trace and audit the same way.

For an agent created some other way, wrap it and install the hooks:

```python
agent = weil_ai.WeilAgent(Agent(...), identity="weil_...")
agent.setup_secure_strands()
```

Sub-agents are secured too: an agent passed in `tools`, or a node of a Graph or
Swarm wrapped with `weil_ai.WeilAgent(graph, ...)`, runs under its own wallet
when it was built with `@weil_ai.agent(...)`, with the parent recorded as its
caller, and under the parent's wallet otherwise.

## A2A

```python
# Serve: callers must sign, and hold Execution or Management on the service.
a2a_app = weil_ai.strands.to_secure_a2a(create_agent_factory, service="engg.acme", port=8201)

# Call: every request is signed with the calling agent's wallet.
tools = [weil_ai.strands.remote_agent_tool(url, name="venues_agent", description="...")]
```

[`event-a2a`](event-a2a) runs three agents this way, the counterpart of the
Google ADK travel example.

## Install

Strands needs its own environment: it pins `mcp` and `a2a-sdk` below the
versions Google ADK uses.

```bash
pip install -e "weil_ai[strands]" "strands-agents[openai]"
```

## Run

The settings are at the top of `secure_hooks.py`. Each is taken from the
environment variable of the same name when that is set, and otherwise from the
value written there, so either fill them in or export them:

```bash
export WEIL_AGENT_IDENTITY=weil_...
export OPENAI_API_KEY=sk-...
python examples/strands/secure_hooks.py
```

Optional: `WEIL_SERVICE` (default `engg.acme`), `OPENAI_MODEL` (default `gpt-5`), and
`SENTINEL_HOST` (default `https://sentinel.weilliptic.ai`; its TLS
certificate is not verified). Keep real keys out of the file if you commit it.

## Tests

`tests/test_strands_hooks.py` and `tests/test_strands_a2a.py` run the hooks,
sub-agents and a real A2A server on localhost against a scripted model, with
the chain calls stubbed, so they need no wallet, Weil network or model key:

```bash
pytest tests
```
