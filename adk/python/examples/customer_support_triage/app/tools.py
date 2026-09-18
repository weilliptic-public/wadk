"""Read-only investigation tools for LLM agents.

Scoped to ONE customer: build_tools(db, customer_id) fixes the ticket's
customer at build time, so tools take no customer_id and order tools refuse
other customers' orders. Only this customer's rows can ever reach the LLM —
no matter what the model (or injected ticket text) asks for.

There are deliberately NO write tools — money moves only through the
limit check in app/agents.py, never via a tool call.
"""
from __future__ import annotations

import json

from langchain_core.tools import tool

from app.db import (
    find_duplicate_charges,
    get_customer,
    get_order,
    get_payments_for_order,
    get_refunds,
    get_tickets_for_customer,
    order_payment_summary,
)


def _norm_order(order_id: str) -> str:
    """Accept '#12345', '12345' or 'ORD-12345' — tickets write it all ways."""
    oid = (order_id or "").strip().upper().lstrip("#").strip()
    return oid if oid.startswith("ORD-") else f"ORD-{oid}"


def build_tools(db, customer_id: str):
    """Bind DB-backed tools scoped to the ticket's customer."""
    cid = (customer_id or "").strip()

    @tool
    def lookup_customer() -> str:
        """Look up THIS ticket's customer account: tier, status, lifetime value, total captured."""
        c = get_customer(db, cid)
        if not c:
            return json.dumps({"found": False, "customer_id": cid})
        captured = sum(o["captured"] for o in order_payment_summary(db, cid))
        return json.dumps({"found": True, "customer_id": c["customer_id"],
                           "tier": c["tier"], "status": c["account_status"],
                           "lifetime_value": c["lifetime_value"],
                           "captured_total": captured})

    @tool
    def list_orders() -> str:
        """List THIS customer's orders with capture counts and captured totals."""
        return json.dumps(order_payment_summary(db, cid))

    @tool
    def order_detail(order_id: str) -> str:
        """Full detail on one of THIS customer's orders: header, every payment
        attempt, refunds on it. Orders belonging to other customers are not
        visible and return found=false."""
        oid = _norm_order(order_id)
        order = get_order(db, oid)
        if not order or order["customer_id"] != cid:
            return json.dumps({"found": False, "order_id": oid})
        payments = get_payments_for_order(db, oid)
        refunds = [r for r in get_refunds(db, cid, days=365)
                   if r["order_id"] == oid]
        return json.dumps({"found": True, "order": order, "payments": payments,
                           "refunds": refunds})

    @tool
    def duplicate_charges(order_id: str) -> str:
        """Succeeded captures beyond the first for one of THIS customer's orders
        (money taken twice). Other customers' orders return nothing."""
        oid = _norm_order(order_id)
        order = get_order(db, oid)
        if not order or order["customer_id"] != cid:
            return json.dumps({"duplicates": [], "duplicate_total": 0.0})
        dups = find_duplicate_charges(db, oid)
        return json.dumps({"duplicates": dups,
                           "duplicate_total": sum(p["amount"] for p in dups)})

    @tool
    def refund_history(days: int = 30) -> str:
        """THIS customer's refunds in the window, plus 14-day counts. Returns
        facts only — YOU decide whether the pattern looks abusive."""
        window = min(max(int(days), 1), 365)
        refs = get_refunds(db, cid, days=window)
        recent = get_refunds(db, cid, days=14)
        return json.dumps({"window_days": window, "refunds": refs,
                           "count_14d": len(recent),
                           "total_14d": sum(r["amount"] for r in recent)})

    @tool
    def past_tickets() -> str:
        """Recent tickets (any status) for THIS customer — repeats, history, context."""
        return json.dumps(get_tickets_for_customer(db, cid))

    return [lookup_customer, list_orders, order_detail,
            duplicate_charges, refund_history, past_tickets]
