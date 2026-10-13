# Setup

Getting `weil-travel-a2a` running from a fresh clone. Each step below exists
because skipping it fails in a way that is hard to diagnose.

## 1. Python 3.12 or 3.13 — not 3.14

`coincurve==21.0.0` has no 3.14 wheel, so pip builds from source and its build
hook fails against `cffi` 2.0.0:

```
RuntimeError: Expected exactly one LICENSE file in cffi distribution, got 0
```

On 3.13 a prebuilt wheel exists and nothing compiles.

```bash
cd python
python3.13 -m venv .venv
```

## 2. Install from the packages, not from requirements.txt

`requirements.txt` contains two `pip freeze` artifacts that point this repo at
itself over HTTPS:

```
-e git+https://github.com/weilliptic-inc/contract-sdk.git@<old sha>#egg=weil_ai&...
```

They prompt for GitHub credentials and pin a commit older than your branch. The
pinned `starlette==0.52.1` also conflicts with `google-adk` 2.x, which makes pip
silently resolve back to `google-adk` 1.10 — a version with no
`a2a/agent/config.py`, so outbound A2A signing cannot work at all.

Install from each package's own metadata instead:

```bash
./.venv/bin/pip install -e ./weil_wallet -e ./weil_ai 'google-adk[a2a,extensions]'
```

Both extras are required, not optional:

| Extra | Provides | Without it |
|---|---|---|
| `a2a` | `a2a-sdk` | `ClientCallContext` unimportable; requests go out unsigned |
| `extensions` | `litellm`, `openai` | `ImportError` on `from google.adk.models.lite_llm import LiteLlm` |

Optional, for editor navigation and linting:

```bash
./.venv/bin/pip install pyright ruff
```

## 3. Agent identities

Each of the three agents needs its own Agent Registry API key, minted with the
platform CLI's `generate_agent_identity`. They are resolved at **import time**,
so a bad key means uvicorn exits before it binds a port.

| Error from Sentinel | Means |
|---|---|
| `invalid API key encoding` | the key string is malformed or truncated |
| `no persisted wallet found for API key` | the key is valid but has no wallet on *this* Sentinel |

A truncated paste gives the first error — the keys are 53 characters.

## 4. Model key

All three agents run on `gpt-4o` through LiteLLM:

```python
model=LiteLlm(model="openai/gpt-4o")
```

so they need `OPENAI_API_KEY`. The model is only called when a request arrives,
so the servers start fine without it and fail on the first question.

To use Gemini instead, change `model=` in all three `agent.py` files to
`"gemini-2.5-flash"` and set `GOOGLE_API_KEY`. The Weil layer wraps the agent,
not the model, so any ADK-supported model works.

## 5. Configuration

One `.env` in this directory serves all three agents. It is gitignored.

```
SENTINEL_HOST=https://your-sentinel-host
OPENAI_API_KEY=sk-...
```

`adk web` loads it automatically (ADK walks up from the agent folder looking for
`.env`). The uvicorn servers need `--env-file .env` passed explicitly, and a
plain `python` invocation needs the variables exported.

`SENTINEL_HOST` defaults to the production endpoint. Pointing it at a host that
does not resolve gives `ConnectError: nodename nor servname provided`; leaving it
unset against a local-only key gives `no persisted wallet found`.

A Sentinel with a self-signed certificate also needs `verify=False` on the
`@weil_ai.agent(...)` decorators. There is no environment variable for it.

## 6. On-chain prerequisites

The gates check the **caller's** wallet, never the callee's.

```python
setup_secure_a2a(service="engg.weil")   # inbound A2A gate
@weil_ai.secured_user("engg.weil")      # tool gate
```

Both resolve `engg.weil` to the `identity::weil` applet and ask
`key_has_purpose` about:

```
engg_<caller's 72-hex canonical address>
```

So the planner's wallet needs **Execution** or **Management** purpose
registered under the `engg` subgroup. Note the service name is parsed as
`<subgroup>.<org>` with a dot — it is not an applet name and is never sent
as-is, so probing `/get_applet_address` for `engg.weil` fails by design.

Check a wallet before running:

```bash
./.venv/bin/python -c "
import asyncio
from weil_ai.mcp import wallet_has_execution_permission as ex
A='<72-hex address>'
print(asyncio.run(ex(A,'engg.weil',sentinel_host='https://your-host',verify=False)))"
```

Without the grant you get `403 Forbidden` from the middleware, or
`WalletNotPermittedError` from the tool gate. Both are the authorization layer
working. Note that a failed *applet resolution* also surfaces as a bare 403, so
if a grant looks correct, verify the service name next.

## 7. Run

Three terminals.

```bash
../../../.venv/bin/uvicorn flights.agent:a2a_app --port 8001 --env-file .env
```

```bash
../../../.venv/bin/uvicorn hotels.agent:a2a_app --port 8002 --env-file .env
```

```bash
../../../.venv/bin/adk web planner/
```

Then open <http://127.0.0.1:8000>, pick `planner`, and ask:

```
Find me hotels in Bombay from October 20 to October 23.
```

Success looks like The Grand Hotel 4.6 / Hotel Central 4.3 / The Royal Palace
4.8 — the hardcoded values in `search_hotels`. Different hotels mean the model
answered without calling the tool; check the hotels terminal for `POST / 200`.

## 8. Stopping a server

Use `Ctrl+C`. `Ctrl+Z` suspends the process while it keeps the port, so the
restart fails with `address already in use` and the suspended server answers
nothing — including its own agent card.

```bash
kill -9 $(lsof -nP -iTCP:8000,8001,8002 -sTCP:LISTEN -t 2>/dev/null)
```

## Troubleshooting

| Symptom | Cause |
|---|---|
| `invalid API key encoding` | malformed or truncated API key |
| `no persisted wallet found for API key` | wrong `SENTINEL_HOST`, or no wallet provisioned |
| `nodename nor servname provided` | `SENTINEL_HOST` does not resolve |
| `ImportError: LiteLLM support requires…` | missing `google-adk[extensions]` |
| `401 Wallet address verification failed` | signature rejected |
| `403 Forbidden` | caller wallet lacks the grant, or the service name does not resolve |
| `WalletNotPermittedError` | same, from the `@secured_user` tool gate |
| `address already in use` | a suspended or orphaned server still holds the port |
| server at 100% CPU, no responses | stale `weil_ai`; restart to pick up the current code |
