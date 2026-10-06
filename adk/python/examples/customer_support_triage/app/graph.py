"""LangGraph orchestrator: intake -> categorizer -> dispatcher -> checks -> resolver.

The dispatcher dynamically spawns sub-agents based on the ticket category
and routing decision. The resolver receives their verdicts to make informed,
database-backed decisions.
"""
from __future__ import annotations

import asyncio
import json
from typing import Literal

from langgraph.graph import END, StateGraph

from app.agents import build_agents
from app.db import get_customer, init_db, log_interaction
from app.models import Resolution, TriageState
from app.rules import load_policy, requested_amount
from app.checks import detect_fraud, diagnose_tech, validate_refund

_AGENTS = None
_POLICY = None
_DB = None
_GRAPH = None


def _db_deps():
    """Cheap deps (policy + sqlite). Never triggers wallet resolution."""
    global _POLICY, _DB
    if _POLICY is None:
        _POLICY = load_policy()
    if _DB is None:
        _DB = init_db()
    return _POLICY, _DB


async def _agent_deps():
    """Full deps. Builds (once) and caches all agent wallets on first use."""
    global _AGENTS
    if _AGENTS is None:
        policy, _ = _db_deps()
        _AGENTS = await build_agents(policy)
    policy, db = _db_deps()
    return _AGENTS, policy, db


def _compiled():
    """Compiled graph, built once (nodes are stateless, no checkpointer)."""
    global _GRAPH
    if _GRAPH is None:
        _GRAPH = build_graph()
    return _GRAPH


def reset_deps() -> None:
    global _AGENTS, _POLICY, _DB, _GRAPH
    _AGENTS, _POLICY, _DB, _GRAPH = None, None, None, None


async def close_deps() -> None:
    """Release cached Weil HTTP clients. Idempotent; safe when unbuilt."""
    global _AGENTS
    agents, _AGENTS = _AGENTS, None
    if not agents:
        return

    await asyncio.gather(*[
        agent.weil_client.close()
        for agent in agents.values()
        if getattr(agent, "weil_client", None) is not None
    ], return_exceptions=True)


# ---------------------------------------------------------------- nodes
async def run_intake(state: TriageState) -> dict:
    ticket = state["ticket"]
    if not ticket.body or not ticket.body.strip():
        return {"halted": True, "halt_reason": "empty ticket body"}

    # First real ticket builds (and caches) all wallets here; halted
    # tickets never pay for wallet resolution.
    agents, policy, db = await _agent_deps()

    customer = get_customer(db, ticket.customer_id)
    log_interaction(db, ticket.ticket_id, "intake", "started")

    # Audit already happens inside the agent via @client.audit() (before run).
    intake_data = await agents["intake"].run(ticket, policy, customer)
    return {"halted": False, "halt_reason": None, "intake_data": intake_data}


async def run_categorizer(state: TriageState) -> dict:
    agents, policy, db = await _agent_deps()
    log_interaction(db, state["ticket"].ticket_id, "categorizer", "started")
    category = await agents["categorizer"].run(state["ticket"], policy)
    return {"category": category}


async def run_router(state: TriageState) -> dict:
    agents, policy, db = await _agent_deps()
    log_interaction(db, state["ticket"].ticket_id, "router", "started")
    routing = await agents["router"].run(state["ticket"], state["category"], policy, db)
    return {"routing": routing}


async def run_dispatcher(state: TriageState) -> dict:
    """Runs deterministic DB checks based on category + routing."""
    _, db = _db_deps()
    ticket = state["ticket"]
    category = state.get("category")
    routing = state.get("routing")

    if not category or not routing:
        return {"sub_agent_verdicts": [], "spawned_agents": []}

    spawned = []
    verdicts = []

    # Billing + refund/credit/waive -> RefundValidator
    if category.primary == "billing":
        body_lower = f"{ticket.subject} {ticket.body}".lower()
        needs_validation = any(w in body_lower for w in (
            "refund", "credit", "waiv", "fee", "charge", "overcharge", "twice", "duplicate"))
        if needs_validation:
            verdict = validate_refund(db, ticket, requested_amount(ticket))
            verdicts.append(verdict.model_dump())
            spawned.append("refund_validator")
            log_interaction(db, ticket.ticket_id, "refund_validator", f"verdict={verdict.verdict}")

    # Fraud detection: always run for billing, or for tickets with suspicious patterns
    if category.primary in ("billing", "account"):
        verdict = detect_fraud(db, ticket)
        verdicts.append(verdict.model_dump())
        spawned.append("fraud_detector")
        log_interaction(db, ticket.ticket_id, "fraud_detector", f"verdict={verdict.verdict}")

    # Technical: always spawn TechDiagnostic
    if category.primary in ("technical", "bug", "how_to"):
        verdict = diagnose_tech(db, ticket)
        verdicts.append(verdict.model_dump())
        spawned.append("tech_diagnostic")
        log_interaction(db, ticket.ticket_id, "tech_diagnostic", f"verdict={verdict.verdict}")

    return {"sub_agent_verdicts": verdicts, "spawned_agents": spawned}


async def run_resolver(state: TriageState) -> dict:
    agents, policy, db = await _agent_deps()
    log_interaction(db, state["ticket"].ticket_id, "resolver", "started")

    resolution = await agents["resolver"].run(
        state["ticket"], state["category"], state["routing"], policy,
        sub_verdicts=state.get("sub_agent_verdicts") or [], db=db)
    return {"resolution": resolution}


async def run_escalate(state: TriageState) -> dict:
    """Terminal policy escalation: every path ends with a Resolution, never None."""
    # Cache hit in-pipeline (router already built agents); builds only on
    # direct calls. Resolver's wallet signs: it owns resolutions.
    agents, _, db = await _agent_deps()

    ticket = state["ticket"]
    routing = state.get("routing")
    team = routing.team if routing else "support"
    log_interaction(db, ticket.ticket_id, "escalate", f"team={team}")
    resolution = Resolution(
        outcome="escalated_to_human", action_attempted="send_response",
        response=f"This needs a human specialist — I've routed it to {team} with full context.",
        resolution_notes=f"escalated to {team} by triage policy",
        authorized=True, confidence=0.9,
        reasoning="policy escalation (non-auto-resolvable or legal trigger)")
    # Anchor the outcome explicitly — no other writer covers this path.
    # Skipped only when no signing client is configured (e.g. stub agents).
    client = getattr(agents.get("resolver"), "weil_client", None)
    if client is not None:
        await client.audit(json.dumps({
            "event": "HUMAN_ESCALATION",
            "agent": "escalate",
            "ticket_id": str(ticket.ticket_id),
            "team": team,
            "reason": getattr(routing, "escalation_reason", None),
        }))
    return {"resolution": resolution}


# ---------------------------------------------------------------- edges

def route_after_intake(state: TriageState) -> Literal["categorizer", "halt"]:
    if state.get("halted"):
        return "halt"
    return "categorizer"


def route_after_category(state: TriageState) -> Literal["router", "halt"]:
    if state.get("halted"):
        return "halt"
    if state.get("category") is None:
        return "halt"
    return "router"


def route_after_routing(state: TriageState) -> Literal["dispatch", "escalate", "halt"]:
    if state.get("halted"):
        return "halt"
    routing = state.get("routing")
    if routing is None:
        return "halt"
    reason = (routing.escalation_reason or "").lower()
    if any(k in reason for k in ("lawsuit", "legal", "chargeback")):
        return "escalate"
    return "dispatch"


def route_after_dispatcher(state: TriageState) -> Literal["resolver", "escalate"]:
    routing = state.get("routing")
    if routing and routing.auto_resolvable:
        return "resolver"
    return "escalate"


def build_graph():
    graph = StateGraph(TriageState)
    graph.add_node("intake", run_intake)
    graph.add_node("categorizer", run_categorizer)
    graph.add_node("router", run_router)
    graph.add_node("dispatcher", run_dispatcher)
    graph.add_node("resolver", run_resolver)
    graph.add_node("escalate", run_escalate)
    graph.set_entry_point("intake")
    graph.add_conditional_edges("intake", route_after_intake,
                                {"categorizer": "categorizer", "halt": END})
    graph.add_conditional_edges("categorizer", route_after_category,
                                {"router": "router", "halt": END})
    graph.add_conditional_edges("router", route_after_routing,
                                {"dispatch": "dispatcher", "escalate": "escalate", "halt": END})
    graph.add_conditional_edges("dispatcher", route_after_dispatcher,
                                {"resolver": "resolver", "escalate": "escalate"})
    graph.add_edge("resolver", END)
    graph.add_edge("escalate", END)
    return graph.compile()


async def run_ticket(ticket) -> dict:
    """Run a single Ticket through the graph; returns the end state."""
    initial: TriageState = {
        "ticket": ticket, "category": None, "routing": None,
        "resolution": None, "halted": False, "halt_reason": None,
        "sub_agent_verdicts": [], "spawned_agents": [],
    }
    return await _compiled().ainvoke(initial)
