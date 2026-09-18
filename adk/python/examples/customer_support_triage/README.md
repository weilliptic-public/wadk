# Customer Support Triage

Ticket triage on LangGraph where each agent has its own Weil wallet, every agent call is
anchored on-chain, and the limits governing a financial action are enforced in code —
never by the model.

Adapts the `classifier → route → responder/human_review` shape
into a five-node graph with per-agent wallet identity, on-chain audit anchoring, and
consequential refund / credit / waiver actions.

## Flow

```
ticket ─┬─ intake ─ categorizer ─ router ─┬─ dispatcher ─ resolver ─ END
        │                                 └─ escalate ─────────────── END
        └─ halt ────────────────────────────────────────────────────── END
```

| Node | Responsibility | LLM |
|---|---|---|
| `intake` | entities, summary, urgency | no — regex |
| `categorizer` | primary/sub-category, sentiment | yes |
| `router` | team, priority, auto-resolve gate | yes |
| `dispatcher` | deterministic DB checks → sub-agent verdicts | no |
| `resolver` | financial action + limit enforcement | yes |
| `escalate` | terminal path for policy / legal escalation | no |

Three LLM calls per ticket. Agents are built once and cached (`graph._agent_deps`), and
released by `close_deps()` on exit.

Agent methods are wrapped in `@client.audit()`, which anchors the call arguments on-chain
*before* the method runs. Outcomes the wrapper cannot see — a layer-2 refusal, a human
escalation — are anchored explicitly at the point they occur.

## Enforcement

`policy_rules.json` is configuration only; it enforces nothing.

- **Layer 1 (soft)** — the system prompts state the limits, so the model knows its
  boundaries. Bypassable by prompt injection.
- **Layer 2 (hard)** — `BaseAgent.check_limits()` (`app/agents.py`) checks the attempted
  amount in code. `infer_limit_key()` maps phrasing (`"refund request"`,
  `"REFUND_APPROVED"`, `"waive all fees"`) onto a limit, and unknown money-moving wording
  falls back to the strictest limit, so novel phrasing cannot slip through.

Over the limit → `authorized=False`, no funds move, refusal anchored on-chain:

```
[4] RESOLUTION    █ BLOCKED BY AUTHORIZATION LIMIT █
                  LLM attempted: issue_refund (amount=$10,000.00)
                  Layer 2 (code): limit check REFUSED → action BLOCKED.
```

## Finding improper requests from evidence

There is no injection keyword list. A request is improper exactly when the order history
cannot justify it — `exceeds_evidence()` in `app/checks.py`:

| Action | Backed by |
|---|---|
| refund | duplicate captures on the order (`actual_overcharge`) |
| fee waiver | a fee the customer was actually charged (`verified_fee`) |
| credit | nothing — every credit exceeds |

A payout is capped at the verified figure, and an account with no captured charges is
escalated rather than paid. The CLI applies one rule to every ticket (`_behaved`): funds
move only up to what the evidence justifies. `--adversarial` selects tickets with this
predicate, not by any naming convention.

## Setup

```bash
pip install -r requirements.txt   # langchain, langgraph, pydantic, python-dotenv
```

Create `.env` in this directory (gitignored):

| Variable | Required | Purpose |
|---|---|---|
| `OPENAI_API_KEY` | yes | live LLM for categorizer / router / resolver |
| `WEIL_API_KEY_{INTAKE,CATEGORIZER,ROUTER,RESOLVER}` | yes | one Agent Registry key per agent, so each wallet identity is distinct |
| `WEIL_S3_ACCESS_KEY`, `WEIL_S3_SECRET_KEY` | if externally stored | sent with the API key on wallet lookup; `AWS_*` names also accepted |
| `WEIL_S3_REGION`, `WEIL_S3_BUCKET` | no | optional S3 location for the same lookup |
| `SENTINEL_HOST` | no | Sentinel base URL; defaults to `weil_wallet.constants.SENTINEL_HOST` |
| `OPENAI_MODEL` | no | defaults to `gpt-4o-mini` |

`SENTINEL_HOST` configures the client's base URL, so it covers wallet lookup,
audit-contract resolution, and signing alike.

Nothing falls back silently: a missing key, or a wallet lookup that fails, raises before
any work is done.

## Run

```bash
python main.py --list                  # ticket IDs
python main.py --ticket TKT-01          # duplicate charge → $120 refunded
python main.py --ticket test_tickets.json:TKT-09
python main.py --demo                  # all 12 tickets
python main.py --adversarial           # only requests the evidence can't justify
pytest tests/test_triage.py -q         # 25 tests; 16 need keys + network
```

`--demo` and `--adversarial` exit non-zero if any ticket misbehaved.

## Tickets

`test_tickets.json` holds twelve fixtures. The seed data (`app/db.py`) gives each one a
definite answer: `ORD-12345` is captured twice at $120, FRE-009 has three refunds in 14
days, `ORD-70001` carries a real $400 duplicate against a $4,000 demand, `ORD-20003` is a
genuine $30 late fee, and FRE-011 has no orders at all. Ticket IDs carry no
classification — which requests are unsupported is worked out from the data.

## Layout

```
policy_rules.json   routing, escalation triggers, agent_limits (config only)
test_tickets.json   8 support cases + 4 evidence-unsupported requests
main.py             CLI + its behaved-as-expected rule
app/models.py       Ticket, Category, Routing, Resolution, SubAgentVerdict, TriageState
app/rules.py        policy loader, team/escalation helpers, requested_amount, infer_limit_key
app/llm.py          live ChatOpenAI client; evidence builder; bounded tool-calling loop
app/prompts.py      system prompts with embedded limits (Layer 1)
app/agents.py       BaseAgent.check_limits (Layer 2) + 4 wallet-backed agents
app/checks.py       deterministic DB checks: validate_refund, exceeds_evidence, fraud, tech
app/db.py           in-memory sqlite schema + seed
app/tools.py        read-only, customer-scoped tools the LLM calls itself
app/graph.py        LangGraph StateGraph, nodes, conditional edges
tests/test_triage.py  9 offline + 16 live graded tests
```

## Not covered

MCP server boundary per agent, persistence across runs, and a web UI.
