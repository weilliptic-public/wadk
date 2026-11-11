# Weil Event Planning A2A (Strands)

Three Strands agents, each with its own Weil wallet, talking over A2A:

| Agent | Runs as | Tools |
|---|---|---|
| `venues_agent` | A2A server on `:8201` | `search_venues`, `hold_venue` (`@secured_user`) |
| `catering_agent` | A2A server on `:8202` | `catering_menus`, `book_catering` (`@secured_user`) |
| `event_planner` | the script you run | the two remote agents, and a local `budget_agent` |

The Strands counterpart of [`google-adk/weil-travel-a2a`](../../google-adk/weil-travel-a2a).

Every agent is a plain Strands `Agent(...)` built inside a `@weil_ai.agent(...)`
factory, as in the ADK example; Weil secures it afterwards.

## What is secured

**Serving side.** Each server is built with:

```python
@weil_ai.agent(identity=os.environ["WEIL_VENUES_IDENTITY"])
def create_venues_agent():
    return Agent(name="venues_agent", model=..., tools=[search_venues, hold_venue])

a2a_app = weil_strands.to_secure_a2a(create_venues_agent, service=SERVICE, port=8201)
```

`to_secure_a2a` builds a fresh agent per A2A conversation from the factory,
installs Weil's hooks on it (ADK: `setup_secure_a2a`), and serves it behind
`WeilA2AMiddleware`. Before the agent runs, every request:

- must carry a valid signature from the caller's wallet (else HTTP 401),
- must come from a wallet holding Execution or Management on `SERVICE` (else 403),
- is audited: `a2a.request.accepted`, `a2a.verify.failed` or `a2a.permission.denied`.

The served agent then runs under its own wallet, and its `strands.agent.*` audit
records carry the caller's wallet as `caller_wallet`. Its `@secured_user` tools
check the serving agent's own wallet, as in the ADK example.

**Calling side.** The planner passes the remote agents to a plain `Agent` as
tools (ADK: `sub_agents=[...]`), then installs Weil's hooks:

```python
@weil_ai.agent(identity=os.environ["WEIL_PLANNER_IDENTITY"])
def create_planner_agent():
    venues_agent = weil_strands.remote_agent_tool(VENUES_URL, name="venues_agent", description="...")
    catering_agent = weil_strands.remote_agent_tool(CATERING_URL, name="catering_agent", description="...")
    return Agent(name="event_planner", model=..., tools=[venues_agent, catering_agent, budget_agent])

weil_planner = create_planner_agent()
weil_planner.setup_secure_strands()   # ADK: setup_secure_a2a()
planner = weil_planner.inner_agent    # the native Strands agent; call planner("...")
```

`remote_agent_tool` must be called inside the factory: that is where it finds the
planner's wallet. Every request to the remote agent is signed with it and audited
(`a2a.request.signed`, `a2a.response.received`). A refusal reaches the planner's
model as the tool's error. Use `weil_strands.remote_agent(url)` instead for the
`A2AAgent` itself, for example as a node in a Graph.

**Sub-agents.** `budget_agent` is a plain Strands `Agent` passed in `tools`, with
no wallet of its own, so it runs as the planner: its runs are audited under the
planner's wallet and its work is part of the planner's trace. An agent built with
its own `@weil_ai.agent(...)` identity would run under that wallet instead, with
the planner recorded as its caller. Graph and Swarm nodes are secured the same way
by `agent.setup_secure_strands()`.

## Running it

Strands needs its own environment (it pins older `mcp` and `a2a-sdk` than
Google ADK):

```bash
pip install -e "../../../weil_ai[strands]" "strands-agents[a2a,openai]"
```

All three agents read their settings from [`settings.py`](settings.py). Each one
comes from the environment variable of the same name when that is set, and
otherwise from the value written in `settings.py`, so either write the values
there or put them in `examples/strands/event-a2a/.env`. Keep real keys out of
`settings.py` if you commit it.

```bash
WEIL_VENUES_IDENTITY=weil_...      # each agent's Weil API key
WEIL_CATERING_IDENTITY=weil_...
WEIL_PLANNER_IDENTITY=weil_...
OPENAI_API_KEY=sk-...
# Optional
WEIL_VENUES_SERVICE=engg.acme      # the service each server gates on
WEIL_CATERING_SERVICE=engg.acme
OPENAI_MODEL=gpt-5
SENTINEL_HOST=https://sentinel.weilliptic.ai   # TLS certificate not verified
```

On chain, the planner's wallet needs Execution (or Management) on both services
to be let in, and the venues and catering wallets need it too for their own
`@secured_user` tools.

Start the servers, from `examples/strands/event-a2a`:

```bash
uvicorn venues.agent:a2a_app --port 8201 --env-file .env
uvicorn catering.agent:a2a_app --port 8202 --env-file .env
```

Then the planner (the first line loads `.env`; skip it if the values are in
`settings.py`):

```bash
set -a; source .env; set +a
python planner/agent.py
```

That starts a chat: type a question at `You:`, read the answer, ask a follow-up
("now hold the second venue"); the planner remembers the conversation. Type
`exit` or press Ctrl+C to quit. Each question is its own run, with its own audit
records and trace. To ask one question and exit instead, put it in the command:

```bash
python planner/agent.py "Plan a team offsite in Pune for 60 people on 12 Dec"
```

To see a refusal, run the planner with a wallet that lacks the permission: the
server answers 403, records `a2a.permission.denied`, and the planner reports
that the agent refused.
