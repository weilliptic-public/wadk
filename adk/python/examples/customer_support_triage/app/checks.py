"""Deterministic DB checks feeding the resolver (no LLM, no agents).

Each function queries the database and returns a structured verdict.
The resolver uses these verdicts (not just ticket text) to make decisions.
"""
from __future__ import annotations

import re
import sqlite3
from datetime import datetime, timezone
from typing import Optional

from app.db import find_duplicate_charges, get_customer, get_order, get_orders, get_refunds
from app.models import SubAgentVerdict


def validate_refund(db: sqlite3.Connection, ticket,
                    amount: Optional[float] = None) -> SubAgentVerdict:
    """Validate a refund request against order history and refund policy."""
    text = f"{ticket.subject} {ticket.body}"
    customer = get_customer(db, ticket.customer_id)
    if not customer:
        return SubAgentVerdict(
            agent="refund_validator", verdict="flagged", confidence=0.9,
            reasoning="Customer not found in database",
            data={"error": "customer_not_found"})

    # No orders on file: there is nothing to verify a refund against,
    # so no refund or waiver can be approved — escalate instead.
    orders = get_orders(db, ticket.customer_id)
    if not orders:
        return SubAgentVerdict(
            agent="refund_validator", verdict="blocked", confidence=0.9,
            reasoning="Customer has no orders on file — no charge to refund, no fees to waive.",
            data={"error": "no_order_history"})

    # Find the referenced order
    order_match = re.search(r"#(\d+)", text)
    order = None
    if order_match:
        order = get_order(db, f"ORD-{order_match.group(1)}")

    # Actual overcharge = duplicate captures on the referenced order.
    # Fallback when the ticket names no order: scan all customer orders so
    # a missing "#12345" can't hide a real duplicate charge.
    actual_overcharge = 0.0
    if order:
        actual_overcharge = sum(
            p["amount"] for p in find_duplicate_charges(db, order["order_id"]))
    else:
        for o in orders:
            actual_overcharge += sum(
                p["amount"] for p in find_duplicate_charges(db, o["order_id"]))

    # If no duplicate found, check if order is within refund window (30 days)
    refundable = True
    if order:
        order_date = datetime.fromisoformat(order["created_at"])
        days_old = (datetime.now(timezone.utc) - order_date).days
        if days_old > 30:
            refundable = False

    # Check refund history
    recent_refunds = get_refunds(db, ticket.customer_id, days=30)
    refund_count = len(recent_refunds)

    # Fees the customer was actually charged — the entitlement behind a waiver.
    # Together with actual_overcharge this is what the DB can justify.
    verified_fee = sum(o["total"] for o in orders
                       if "fee" in (o.get("description") or "").lower())

    data = {
        "customer_tier": customer["tier"],
        "customer_name": customer["name"],
        "order_found": order is not None,
        "order_amount": order["total"] if order else None,
        "order_status": order["status"] if order else None,
        "actual_overcharge": actual_overcharge,
        "verified_fee": verified_fee,
        "refundable": refundable,
        "recent_refund_count": refund_count,
    }

    if actual_overcharge > 0 and amount and amount <= actual_overcharge:
        return SubAgentVerdict(
            agent="refund_validator", verdict="approved", confidence=0.95,
            reasoning=f"Order shows ${actual_overcharge:.2f} overcharge (duplicate charge). "
                      f"Requested ${amount:.2f} matches actual overcharge amount.",
            data=data)

    if actual_overcharge > 0 and amount and amount > actual_overcharge:
        return SubAgentVerdict(
            agent="refund_validator", verdict="flagged", confidence=0.85,
            reasoning=f"Order shows ${actual_overcharge:.2f} actual overcharge, "
                      f"but ${amount:.2f} requested — ${amount - actual_overcharge:.2f} overcharge.",
            data={**data, "overcharge_delta": amount - actual_overcharge})

    if order and refundable:
        return SubAgentVerdict(
            agent="refund_validator", verdict="approved", confidence=0.8,
            reasoning=f"Order {order['order_id']} found (${order['total']:.2f}), "
                       f"within 30-day refund window.",
            data=data)

    if not refundable:
        return SubAgentVerdict(
            agent="refund_validator", verdict="blocked", confidence=0.9,
            reasoning="Order is outside 30-day refund window.",
            data=data)

    # No order referenced and no duplicate charge found. For money the safe
    # default is "cannot verify", so this approves only when the DB still backs
    # the amount — a fee the customer was really charged. The comparison stays
    # inline because exceeds_evidence() calls this function.
    if verified_fee > 0 and amount and 0 < amount <= verified_fee:
        return SubAgentVerdict(
            agent="refund_validator", verdict="approved", confidence=0.75,
            reasoning=f"No duplicate charge found, but this account was charged "
                      f"${verified_fee:.2f} in fees, which covers the requested "
                      f"${amount:.2f}.",
            data=data)

    return SubAgentVerdict(
        agent="refund_validator", verdict="blocked", confidence=0.85,
        reasoning="No order referenced and no duplicate charge found — nothing on "
                  "file backs this request.",
        data=data)


def exceeds_evidence(db: sqlite3.Connection, ticket,
                     requested: Optional[float]) -> bool:
    """True when a ticket asks for more money than the order history justifies.

    This is the evidence-based replacement for matching injection keywords: a
    request is improper exactly when nothing in the database backs it. A refund
    is backed by duplicate captures, a waiver by a fee the customer was really
    charged, and a credit by nothing at all.
    """
    if not requested or requested <= 0:
        return False
    data = validate_refund(db, ticket, requested).data or {}
    justified = max(
        float(data.get("actual_overcharge") or 0.0),
        float(data.get("verified_fee") or 0.0),
    )
    return requested > justified


def detect_fraud(db: sqlite3.Connection, ticket) -> SubAgentVerdict:
    """Check for suspicious patterns in customer history."""
    customer = get_customer(db, ticket.customer_id)
    if not customer:
        return SubAgentVerdict(
            agent="fraud_detector", verdict="clear", confidence=0.5,
            reasoning="Customer not found; cannot assess fraud risk.")

    # Check refund velocity (count in last 14 days)
    recent = get_refunds(db, ticket.customer_id, days=14)
    refund_count_14d = len(recent)
    refund_total_14d = sum(r["amount"] for r in recent)

    # Check if customer is new (< 30 days)
    created = datetime.fromisoformat(customer["created_at"])
    age_days = (datetime.now(timezone.utc) - created).days
    is_new = age_days < 30

    # Check if multiple refunds on same order
    refund_order_ids = [r["order_id"] for r in recent]
    multi_refund_orders = len(refund_order_ids) - len(set(refund_order_ids))

    data = {
        "customer_tier": customer["tier"],
        "customer_age_days": age_days,
        "refund_count_14d": refund_count_14d,
        "refund_total_14d": refund_total_14d,
        "multi_refund_orders": multi_refund_orders,
    }

    # Flag: 3+ refunds in 14 days
    if refund_count_14d >= 3:
        return SubAgentVerdict(
            agent="fraud_detector", verdict="flagged", confidence=0.8,
            reasoning=f"Suspicious: {refund_count_14d} refunds in 14 days "
                      f"(${refund_total_14d:.2f} total). Pattern suggests abuse.",
            data=data)

    # Flag: new customer + large refund request
    if is_new and refund_total_14d > 100:
        return SubAgentVerdict(
            agent="fraud_detector", verdict="flagged", confidence=0.7,
            reasoning=f"New customer ({age_days} days) with ${refund_total_14d:.2f} "
                      f"in refunds within 14 days.",
            data=data)

    # Flag: multiple refunds on same order
    if multi_refund_orders > 0:
        return SubAgentVerdict(
            agent="fraud_detector", verdict="flagged", confidence=0.75,
            reasoning="Multiple refunds on same order(s) detected.",
            data=data)

    return SubAgentVerdict(
        agent="fraud_detector", verdict="clear", confidence=0.85,
        reasoning="No suspicious patterns in customer refund history.",
        data=data)


def diagnose_tech(db: sqlite3.Connection, ticket) -> SubAgentVerdict:
    """Gather technical context for technical/bug tickets."""
    customer = get_customer(db, ticket.customer_id)
    text = f"{ticket.subject} {ticket.body}"

    # Check for historical tickets on same topic
    rows = db.execute(
        "SELECT ticket_id, subject, status FROM tickets WHERE customer_id = ? "
        "AND ticket_id != ? ORDER BY created_at DESC LIMIT 5",
        (ticket.customer_id, ticket.ticket_id)).fetchall()
    related = [dict(r) for r in rows]

    # Check if customer has pending orders (might be related to integration issues)
    orders = get_orders(db, ticket.customer_id)
    pending = [o for o in orders if o.get("status") == "pending"]

    data = {
        "customer_tier": customer["tier"] if customer else "unknown",
        "related_tickets": len(related),
        "related_ticket_ids": [t["ticket_id"] for t in related],
        "pending_orders": len(pending),
    }

    # Diagnose common patterns
    if "401" in text or "unauthorized" in text.lower():
        return SubAgentVerdict(
            agent="tech_diagnostic", verdict="diagnosed", confidence=0.85,
            reasoning="401 Unauthorized: likely API key or permission issue. "
                      "Check API credentials and access scope.",
            data={**data, "error_type": "authentication", "suggested_fix": "verify_api_key"})

    if "crash" in text.lower():
        return SubAgentVerdict(
            agent="tech_diagnostic", verdict="diagnosed", confidence=0.7,
            reasoning="Crash reported. No related crash tickets from this customer.",
            data={**data, "error_type": "crash", "suggested_fix": "check_logs"})

    if "locked out" in text.lower():
        return SubAgentVerdict(
            agent="tech_diagnostic", verdict="diagnosed", confidence=0.8,
            reasoning="Team lockout: check for SSO/IdP misconfiguration or "
                      "expired session tokens.",
            data={**data, "error_type": "lockout", "suggested_fix": "reset_sso"})

    return SubAgentVerdict(
        agent="tech_diagnostic", verdict="clear", confidence=0.6,
        reasoning="No specific technical diagnosis from database context.",
        data=data)
