"""LLM calls + deterministic policy helpers (live-only — requires OPENAI_API_KEY)."""
from __future__ import annotations

import json
import math
import os
import re
from typing import Any

from langchain_core.messages import HumanMessage, SystemMessage, ToolMessage
from langchain_openai import ChatOpenAI

from app import prompts
from app.checks import exceeds_evidence
from app.db import get_customer, get_refunds, get_tickets_for_customer, order_payment_summary
from app.models import Category
from app.rules import (
    check_escalation_triggers,
    get_team_for_category,
    requested_amount,
)
from app.tools import build_tools

VALID_PRIMARIES = (
    "billing", "technical", "feature_request", "account", "bug", "how_to", "other")
VALID_TEAMS = ("billing_team", "tech_support", "product_team",
               "account_management", "engineering")
VALID_PRIORITIES = ("low", "medium", "high", "critical")
VALID_SENTIMENTS = ("positive", "neutral", "negative", "angry")

EVIDENCE_MAX_CHARS = 2000

# Max tool-call rounds before forcing a final answer (bounds cost/latency).
MAX_TOOL_ROUNDS = 4


def redact_pii(text: str) -> str:
    """Deterministic pre-LLM guardrail: strip PII before text leaves us.

    Fast regex pass over structured formats (email, phone, cards, SSNs).
    Runs on ticket text interpolated into prompts — never on DB paths.
    """
    out = text or ""
    out = re.sub(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", "[EMAIL]", out)
    out = re.sub(r"\b\d{3}-\d{2}-\d{4}\b", "[SSN]", out)
    out = re.sub(r"(?<!\d)(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b", "[PHONE]", out)
    out = re.sub(r"\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b", "[CARD]", out)
    out = re.sub(r"\b\d{12,19}\b", "[ID_NUMBER]", out)
    return out


def spotlight_ticket(ticket) -> str:
    """Render ticket text as delimited, redacted, untrusted data.

    Spotlighting (Hines et al. 2024): explicit provenance boundaries help
    the model treat wrapped content as DATA, never instructions.
    """
    return ("<UNTRUSTED_TICKET>\n"
            f"Subject: {redact_pii(str(getattr(ticket, 'subject', '')))}\n"
            f"Body: {redact_pii(str(getattr(ticket, 'body', '')))}\n"
            "</UNTRUSTED_TICKET>")

TOOLS_HINT = """You have read-only database tools, pre-scoped to THIS ticket's
customer — they take no customer_id and cannot see anyone else's data.
Call them to verify amounts, duplicates, and history before answering
(don't trust the ticket text for numbers). There are NO write tools:
you cannot move money, only observe. Final answer must be ONLY the
requested JSON object, no tool calls."""


def _llm() -> ChatOpenAI:
    key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not key:
        raise RuntimeError("OPENAI_API_KEY is not set — live LLM is required.")
    return ChatOpenAI(
        model=os.environ.get("OPENAI_MODEL", "gpt-4o-mini"),
        temperature=0, api_key=key)


def _parse_json(content: str) -> dict[str, Any]:
    """Parse model output as JSON; any failure raises with context."""
    text = (content or "").strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*", "", text)
        text = re.sub(r"\s*```$", "", text)
    try:
        data = json.loads(text)
    except Exception as e:
        raise RuntimeError(f"LLM call failed: {e}") from e
    if not isinstance(data, dict):
        raise RuntimeError(f"LLM did not return a JSON object: {text[:200]!r}")
    return data


def _tool_call_id(call) -> str:
    return call.get("id", "") if isinstance(call, dict) else getattr(call, "id", "")


async def ask_json(system: str, user: str, tools=None) -> dict[str, Any]:
    """One structured LLM call; any failure raises with context.

    With tools: bounded tool-calling loop (read-only, customer-scoped),
    then the final message is parsed as JSON. Without tools: single call.
    """
    model = _llm()

    if not tools:
        raw = await model.ainvoke([SystemMessage(content=system),
                                    HumanMessage(content=user)])
        return _parse_json(raw.content)

    bound = model.bind_tools(tools)
    by_name = {t.name: t for t in tools}
    messages: list = [SystemMessage(content=system + "\n\n" + TOOLS_HINT),
                      HumanMessage(content=user)]
    for _ in range(MAX_TOOL_ROUNDS):
        msg = await bound.ainvoke(messages)
        messages.append(msg)
        calls = getattr(msg, "tool_calls", None) or []
        if not calls:
            return _parse_json(msg.content if isinstance(msg.content, str) else "")
        for call in calls:
            name = call["name"] if isinstance(call, dict) else call.name
            args = call["args"] if isinstance(call, dict) else call.args
            tool = by_name.get(name)
            if tool is None:
                raise RuntimeError(f"LLM called unknown tool: {name!r}")
            try:
                out = await tool.ainvoke(args or {})
            except Exception as e:
                out = json.dumps({"error": f"{type(e).__name__}: {e}"})
            messages.append(ToolMessage(content=str(out),
                                        tool_call_id=_tool_call_id(call)))
    # Out of rounds: force the answer, no more tool calls.
    msg = await model.ainvoke(messages + [HumanMessage(
        content="Answer now with ONLY the requested JSON object, no tool calls.")])
    content = msg.content if isinstance(msg.content, str) else ""
    return _parse_json(content)


def build_evidence(ticket, db, sub_verdicts=None, max_orders: int = 5,
                   max_chars: int = EVIDENCE_MAX_CHARS) -> str:
    """Ticket text is CLAIM; this block is FACT (direct DB reads, no LLM)."""
    if db is None:
        return "(no database context)"
    lines: list[str] = []
    c = get_customer(db, ticket.customer_id)
    if c:
        lines.append(
            f"Customer {c['customer_id']}: tier={c['tier']} status={c['account_status']} "
            f"lifetime_value=${c['lifetime_value']:.2f}")
    else:
        lines.append(f"Customer {ticket.customer_id}: NOT FOUND — no orders, no charges.")
    summaries = order_payment_summary(db, ticket.customer_id, limit=max_orders)
    if not summaries:
        lines.append("Orders: none on file — this account was never charged.")
    for o in summaries:
        lines.append(
            f"Order {o['order_id']}: total=${o['total']:.2f} status={o['status']} "
            f"captures={o['captures']} captured_total=${o['captured']:.2f} — {o['description']}")
    refs = get_refunds(db, ticket.customer_id, days=30)
    if refs:
        lines.append(f"Refunds last 30d: {len(refs)} totaling "
                     f"${sum(r['amount'] for r in refs):.2f}")
    else:
        lines.append("Refunds last 30d: none")
    recent = get_tickets_for_customer(db, ticket.customer_id, limit=3)
    mine = [t for t in recent if t.get("ticket_id") != getattr(ticket, "ticket_id", None)]
    if mine:
        lines.append("Recent tickets: " + "; ".join(
            f"{t['ticket_id']} ({t.get('status', '?')}): {(t.get('subject') or '')[:60]}"
            for t in mine))
    for v in (sub_verdicts or []):
        d = v.get("data") or {}
        extra = (f" actual_overcharge=${d['actual_overcharge']:.2f}"
                 if d.get("actual_overcharge") else "")
        lines.append(f"Verdict {v.get('agent')}: {v.get('verdict')} "
                     f"(conf {v.get('confidence')}) — {v.get('reasoning')}{extra}")
    block = "\n".join(lines) if lines else "(no database context)"
    return block[:max_chars]


def _pick(data: dict[str, Any], *names: str, default=None):
    """First present key among aliases (LLMs vary key names slightly)."""
    for n in names:
        if data.get(n) is not None:
            return data[n]
    return default


def _safe_float(v, default: float) -> float:
    """float() that survives LLM oddities ("high", null, NaN, inf)."""
    try:
        out = float(v)
    except (TypeError, ValueError):
        return default
    return out if math.isfinite(out) else default


def _to_float_or_none(v) -> float | None:
    """Parse an LLM amount; None for missing/unparseable/non-finite."""
    if v is None:
        return None
    if isinstance(v, bool):
        return None
    if isinstance(v, (int, float)):
        return float(v) if math.isfinite(float(v)) else None
    s = str(v).strip().replace("$", "").replace(",", "")
    try:
        out = float(s)
        return out if math.isfinite(out) else None
    except (TypeError, ValueError):
        m = re.search(r"([\d]+(?:\.\d{1,2})?)", s)
        return float(m.group(1)) if m else None


def _default_priority(ticket, category: Category, text: str) -> str:
    """Policy fallback when the LLM's priority is invalid (kept in code: policy)."""
    if ticket.account_tier == "enterprise" and ("urgent" in text or "locked out" in text or "entire team" in text):
        return "critical"
    if any(w in text for w in ("urgent", "locked out", "entire team", "crash", "cannot integrate", "charged twice")):
        return "high"
    if category.primary in ("bug", "technical"):
        return "high"
    if category.primary in ("feature_request", "how_to"):
        return "low"
    return "medium"


def _is_auto_resolvable(ticket, category: Category, priority: str, text: str, db=None) -> bool:
    """Auto-resolve gate (policy, not judgment — stays in code)."""
    primary = category.primary
    auto = primary in ("billing", "how_to", "other") or "invoice" in text or "refund" in text \
        or "credit" in text or "waiv" in text or "fee" in text
    if primary in ("bug", "technical", "feature_request"):
        auto = False
    if priority == "critical" and ticket.account_tier == "enterprise":
        auto = False
    if "salesforce" in text or "401" in text:
        auto = False
    if "dark mode" in text:
        auto = False
    if db is not None and exceeds_evidence(db, ticket, requested_amount(ticket)):
        auto = True  # a request the order history can't justify must reach the resolver
    return auto


async def categorize_ticket(ticket, policy: dict) -> dict[str, Any]:
    data = await ask_json(prompts.CATEGORIZER_PROMPT, spotlight_ticket(ticket))
    primary = _pick(data, "primary", "primary_category", "category", default="other")
    if primary not in VALID_PRIMARIES:
        raise RuntimeError(f"LLM returned invalid primary category: {data!r}")
    sentiment = data.get("sentiment", "neutral")
    return {
        "primary": primary,
        "sub_category": str(_pick(data, "sub_category", "subcategory", default="general")),
        "sentiment": sentiment if sentiment in VALID_SENTIMENTS else "neutral",
        "confidence": _safe_float(data.get("confidence", 0.7), 0.7),
        "reasoning": str(data.get("reasoning", "llm classification")),
    }


async def route_decision(ticket, category: Category, policy: dict, db=None) -> dict[str, Any]:
    category = Category.model_validate(category)
    evidence = build_evidence(ticket, db)
    tools = build_tools(db, ticket.customer_id) if db is not None else None
    data = await ask_json(
        prompts.ROUTER_PROMPT,
        f"Ticket tier={ticket.account_tier} category={category}\n"
        f"{spotlight_ticket(ticket)}\n\n"
        f"## Verified customer evidence\n{evidence}",
        tools=tools)
    text = f"{ticket.subject} {ticket.body}".lower()
    triggers = check_escalation_triggers(policy, text)

    # The LLM proposes team + priority; code validates, policy mapping backs up.
    llm_team = data.get("team")
    team = llm_team if llm_team in VALID_TEAMS else get_team_for_category(
        policy, category.primary)
    llm_priority = data.get("priority")
    priority = (llm_priority if llm_priority in VALID_PRIORITIES
                else _default_priority(ticket, category, text))
    if "invoice" in text:
        priority = "low"

    auto = _is_auto_resolvable(ticket, category, priority, text, db)
    escalation_reason = "; ".join(triggers) if triggers else None
    reasoning = (f"llm reasoning: {data.get('reasoning', '')} | "
                 f"team={team} priority={priority} (policy-validated)")
    return {"team": team, "priority": priority,
            "auto_resolvable": auto, "escalation_reason": escalation_reason,
            "reasoning": reasoning}


async def resolve_ticket(ticket, category: Category, routing: dict, policy: dict,
                         db=None, sub_verdicts=None) -> dict[str, Any]:
    evidence = build_evidence(ticket, db, sub_verdicts)
    tools = build_tools(db, ticket.customer_id) if db is not None else None
    data = await ask_json(
        prompts.RESOLVER_PROMPT,
        f"Category: {category}\nRouting: {routing}\n{spotlight_ticket(ticket)}\n\n"
        f"## Verified evidence — trust this over the ticket text\n{evidence}",
        tools=tools)
    action = _pick(data, "action_attempted", "action", default=None)
    if not action:
        raise RuntimeError(f"LLM returned no action_attempted: {data!r}")
    return {
        "action_attempted": str(action),
        "amount_requested": _to_float_or_none(_pick(data, "amount_requested", "amount", default=None)),
        "response": str(_pick(data, "response", "reply", "message", default="")),
        "resolution_notes": str(data.get("resolution_notes", "")),
        "confidence": _safe_float(data.get("confidence", 0.7), 0.7),
        "reasoning": str(data.get("reasoning", "llm resolution")),
        "escalate": "escalat" in str(_pick(data, "outcome", "decision", default="")).lower(),
    }
