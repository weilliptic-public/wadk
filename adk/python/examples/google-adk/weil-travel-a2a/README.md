# Weil Travel A2A

This mirrors `examples/google-adk/travel-a2a`, but every ADK agent is wrapped
with a Weil identity.

The serving agents call:

```python
weil_agent.setup_secure_a2a(service="travel::flights")
a2a_app = weil_agent.to_secure_a2a(host="127.0.0.1", port=8001)
```

Tools can be guarded with the same `@secured_user(...)` decorator used by MCP:

```python
@weil_ai.secured_user("engg.weil")
def search_flights(origin: str, destination: str, date: str) -> str:
    ...
```

The planner calls:

```python
weil_root_agent.setup_secure_a2a()
root_agent = weil_root_agent.inner_agent
```

`setup_secure_a2a()` patches `RemoteA2aAgent` sub-agents with outbound A2A
request interceptors. Each remote call is signed with the planner wallet and
audited after a response is received.

`to_secure_a2a()` wraps ADK's `to_a2a(...)` app with inbound verification
middleware. The middleware verifies the caller signature, checks execution
permission for the configured service, stores the verified caller wallet in
context for `@secured_user(...)` tools, and audits accepted or rejected calls.

## Running it

See [SETUP.md](SETUP.md) for the full prerequisites: Python version, the two
required `google-adk` extras, agent-registry keys, the model key, and the
on-chain grants the gates check.

Run the remote agents first:

```bash
uvicorn flights.agent:a2a_app --port 8001 --env-file .env
uvicorn hotels.agent:a2a_app --port 8002 --env-file .env
```

Then the planner:

```bash
adk web planner/
```

`--env-file .env` matters for the uvicorn servers: `adk web` finds `.env` on its
own, uvicorn does not.
