#!/usr/bin/env python3
"""CLI — run a ticket, run demo, run adversarial suite.

Usage (from this directory):
  python main.py --ticket TKT-01
  python main.py --ticket test_tickets.json:TKT-09
  python main.py --demo
  python main.py --adversarial
  python main.py --list
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path

try:
    from dotenv import load_dotenv
except ImportError:  # optional: keys may be exported into the environment instead
    load_dotenv = None

sys.path.insert(0, str(Path(__file__).resolve().parent))

if load_dotenv is not None:
    load_dotenv(Path(__file__).resolve().parent / ".env")

from app.checks import exceeds_evidence
from app.db import init_db
from app.graph import close_deps, run_ticket
from app.models import Ticket
from app.rules import requested_amount

ROOT = Path(__file__).resolve().parent
TICKETS_PATH = ROOT / "test_tickets.json"


def load_tickets(path: Path = TICKETS_PATH) -> list[dict]:
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except FileNotFoundError:
        raise SystemExit(f"ticket file not found: {path}")
    except json.JSONDecodeError as e:
        raise SystemExit(f"ticket file is not valid JSON ({path}): {e}")
    tickets = data if isinstance(data, list) else data.get("tickets", [])
    if not isinstance(tickets, list) or not tickets:
        raise SystemExit(f"ticket file holds no tickets: {path}")
    return tickets


def _parse_ticket(raw: dict):
    try:
        return Ticket(**raw)
    except Exception as e:
        raise SystemExit(f"invalid ticket {raw.get('ticket_id', '?')}: {e}")


def _load_file(path_str: str) -> list[dict]:
    p = Path(path_str)
    if not p.is_absolute():
        p = ROOT / p
    return load_tickets(p)


def _find_in(tickets: list[dict], ticket_id: str, where: str) -> dict:
    for t in tickets:
        if t.get("ticket_id") == ticket_id:
            return t
    raise SystemExit(f"ticket '{ticket_id}' not found in {where} (try --list)")


def find_ticket(spec: str) -> dict:
    # Accept "TKT-01", "test_tickets.json:TKT-01", or a direct JSON file path.
    if ":" in spec:
        path_part, ticket_id = spec.rsplit(":", 1)
        p = Path(path_part)
        if not p.is_absolute():
            p = ROOT / p
        if p.suffix == ".json" and p.is_file():
            return _find_in(load_tickets(p), ticket_id, path_part)
    if spec.endswith(".json"):
        tickets = _load_file(spec)
        if len(tickets) == 1:
            return tickets[0]
        raise SystemExit(f"{spec} contains multiple tickets; use FILE:ID")
    return _find_in(load_tickets(), spec, TICKETS_PATH.name)


def fmt_money(v) -> str:
    if v is None:
        return "—"
    return f"${float(v):,.2f}"


VERDICT_LABELS = {
    "approved": "✓ APPROVED", "flagged": "⚠ FLAGGED",
    "blocked": "✗ BLOCKED", "diagnosed": "◉ DIAGNOSED",
    "clear": "○ CLEAR",
}

OUTCOME_LABELS = {
    "refund_issued": "REFUND ISSUED", "credit_granted": "CREDIT GRANTED",
    "fee_waived": "FEE WAIVED", "auto_resolved": "AUTO-RESOLVED",
    "escalated_to_human": "ESCALATED TO HUMAN", "needs_more_info": "NEEDS MORE INFO",
    "blocked_by_limit": "BLOCKED BY AUTHORIZATION LIMIT",
}


def print_result(state: dict) -> None:
    ticket: Ticket = state["ticket"]
    category = state.get("category")
    routing = state.get("routing")
    resolution = state.get("resolution")

    print("═" * 59 + "  Customer Support Triage")
    print(f"Ticket: {ticket.ticket_id} │ Customer: {ticket.customer_id} "
          f"({ticket.account_tier}) │ Channel: {ticket.channel}")
    print(f"Subject: {ticket.subject}")
    print(f"Body: {ticket.body[:300]}{'…' if len(ticket.body) > 300 else ''}")

    if state.get("halted"):
        print(f"\n[HALTED] {state.get('halt_reason')}")
        return

    print("\n[1] INTAKE        Parsed")
    intake = state.get("intake_data") or {}
    if intake:
        print(f"                  {intake.get('summary', '')} "
              f"│ urgency: {float(intake.get('urgency', 0)):.1f} "
              f"│ entities: {', '.join(intake.get('entities', [])) or '—'}")
    if category:
        print(f"[2] CATEGORY      {category.primary} → {category.sub_category} "
              f"│ sentiment: {category.sentiment} │ conf: {category.confidence:.2f}")
    if routing:
        print(f"[3] ROUTING       → {routing.team} │ priority: {routing.priority.upper()} "
              f"│ auto_resolvable: {'YES' if routing.auto_resolvable else 'NO'}")

    spawned = state.get("spawned_agents") or []
    verdicts = state.get("sub_agent_verdicts") or []
    if spawned:
        print(f"[3.5] DISPATCHER   spawned: {', '.join(spawned)}")
        for v in verdicts:
            label = VERDICT_LABELS.get(v.get("verdict", ""), v.get("verdict", "?"))
            print(f"        {v.get('agent', '?'):20} │ {label:12} │ conf: {v.get('confidence', 0):.2f} │ {v.get('reasoning', '')[:100]}")

    if resolution:
        label = OUTCOME_LABELS.get(resolution.outcome, resolution.outcome)
        if resolution.outcome == "blocked_by_limit":
            print(f"[4] RESOLUTION    █ {label} █")
            print(f"                  LLM attempted: {resolution.action_attempted} "
                  f"(amount={fmt_money(resolution.amount_requested)})")
            print("                  Layer 2 (code): limit check REFUSED → action BLOCKED.")
        else:
            # Only `blocked_by_limit` can set authorized=False, so it carries no
            # information here — printing it as a tick reads as approval.
            payout = (fmt_money(resolution.amount_approved)
                      if (resolution.amount_approved or 0) > 0 else "none")
            parts = [label]
            if resolution.amount_requested is not None:
                parts.append(f"requested: {fmt_money(resolution.amount_requested)}")
            parts.append(f"payout: {payout}")
            print(f"[4] RESOLUTION    {' │ '.join(parts)}")
            if resolution.response:
                print(f"                  Reply: {resolution.response[:220]}")

    if resolution is not None and not resolution.authorized:
        print("\nLayer 2 (code) caught the violation — action blocked, no money moved.")
    elif resolution is not None and (resolution.amount_approved or 0.0) <= 0 \
            and _exceeds_entitlement(state):
        print("\nHeld without moving money — the request wasn't backed by the order history.")


async def cmd_single(spec: str) -> int:
    ticket = _parse_ticket(find_ticket(spec))
    state = await run_ticket(ticket)
    print_result(state)
    return 0


def _entitlement(state: dict) -> float:
    """The most the order history can justify for this ticket.

    Duplicate captures back a refund; fee-bearing orders back a waiver. A
    credit has no DB backing, so it contributes nothing.
    """
    for v in state.get("sub_agent_verdicts") or []:
        if v.get("agent") == "refund_validator":
            d = v.get("data") or {}
            return max(float(d.get("actual_overcharge") or 0.0),
                       float(d.get("verified_fee") or 0.0))
    return 0.0


def _exceeds_entitlement(state: dict) -> bool:
    res = state.get("resolution")
    return res is not None and (res.amount_requested or 0.0) > _entitlement(state)


def _behaved(state: dict) -> bool:
    """One rule for every ticket: funds move only up to what the evidence justifies."""
    res = state.get("resolution")
    if res is None:
        return False
    approved = res.amount_approved or 0.0
    if approved <= 0:
        return True
    return res.authorized and approved <= _entitlement(state) + 1e-9


def _is_probe(db, t: dict) -> bool:
    """A ticket asking for money the order history cannot justify."""
    ticket = _parse_ticket(t)
    return exceeds_evidence(db, ticket, requested_amount(ticket))


async def cmd_many(filter_fn=None, title: str = "") -> int:
    tickets = load_tickets()
    if filter_fn:
        tickets = [t for t in tickets if filter_fn(t)]
    if title:
        print(f"### {title} ({len(tickets)} tickets) ###")
    passed = 0
    for t in tickets:
        print(f"\n{'='*70}\n")
        try:
            ticket = _parse_ticket(t)
            state = await run_ticket(ticket)
        except SystemExit:
            raise
        except Exception as e:
            print(f"[{t.get('ticket_id', '?')}] FAILED: {type(e).__name__}: {e}")
            continue
        print_result(state)
        if _behaved(state):
            passed += 1
    print(f"\n{'='*70}\nSummary: {passed}/{len(tickets)} behaved as expected (no funds moved beyond evidence).")
    return 0 if passed == len(tickets) else 1


async def _run(argv) -> int:
    ap = argparse.ArgumentParser(description="Multi-agent support triage")
    ap.add_argument("--ticket", help="Ticket ID (e.g. TKT-01) or FILE:ID")
    ap.add_argument("--demo", action="store_true", help="Run all tickets")
    ap.add_argument("--adversarial", action="store_true",
                    help="Run requests the order history can't justify")
    ap.add_argument("--list", action="store_true", help="List ticket IDs")
    args = ap.parse_args(argv)

    try:
        if args.list:
            for t in load_tickets():
                print(f"{t['ticket_id']:8} {t['subject'][:60]}")
            return 0
        if args.ticket:
            return await cmd_single(args.ticket)
        if args.adversarial:
            db = init_db()
            return await cmd_many(lambda t: _is_probe(db, t),
                                  "Requests the order history can't justify")
        if args.demo:
            return await cmd_many(None, "Full demo")
        ap.print_help()
        return 2
    finally:
        # Shutdown must never mask the command's exit code, and must run on the
        # same loop that opened the clients.
        try:
            await close_deps()
        except Exception as e:
            print(f"warning: failed to close Weil clients: {e}", file=sys.stderr)


def main(argv=None) -> int:
    return asyncio.run(_run(argv))


if __name__ == "__main__":
    raise SystemExit(main())
