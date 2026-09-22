"""4 agents with two-layer enforcement.

Layer 1 (soft): system prompts include limits so the LLM knows its boundaries.
Layer 2 (hard): BaseAgent.check_limits() checks amounts against policy_rules.json in
code, outside the LLM. If exceeded -> authorized=False and the action is blocked.

Audit: each agent is audited via WeilClient from weil_wallet.
"""
from __future__ import annotations

import asyncio
import json
import os
import re
from typing import Optional
from weil_wallet import WeilClient
from app.db import customer_captured_total
from app.llm import _safe_float, _to_float_or_none, categorize_ticket, resolve_ticket, route_decision
from app.rules import get_agent_limits, infer_limit_key
from app.prompts import CATEGORIZER_PROMPT, INTAKE_PROMPT, ROUTER_PROMPT, build_resolver_prompt
from app.models import Category, Routing, Resolution

def _creds_from_env() -> Optional[dict]:
    """Build the S3 credentials dict from env vars, or None if unset.

    Supports the triage ``WEIL_S3_*`` names (see README/``.env``) with
    ``AWS_*`` fallbacks (see ``examples/langchain_apikey.py``).
    """
    access_key = os.environ.get("WEIL_S3_ACCESS_KEY") or os.environ.get("AWS_ACCESS_KEY_ID")
    secret_key = os.environ.get("WEIL_S3_SECRET_KEY") or os.environ.get("AWS_SECRET_ACCESS_KEY")
    if not access_key or not secret_key:
        return None
    creds: dict = {
        "access_key_id": access_key,
        "secret_access_key": secret_key,
    }
    region = (
        os.environ.get("WEIL_S3_REGION")
        or os.environ.get("AWS_REGION")
    )
    bucket = (
        os.environ.get("WEIL_S3_BUCKET")
        or os.environ.get("AWS_S3_BUCKET")
        or os.environ.get("AWS_BUCKET_NAME")
    )
    if region:
        creds["region"] = region
    if bucket:
        creds["bucket_name"] = bucket
    return creds

def _sentinel_host_from_env() -> Optional[str]:
    """Sentinel base URL from ``SENTINEL_HOST``, or None to use the SDK default.

    The host also backs wallet lookup, audit-contract resolution, and signing,
    since all three go through the client's base URL.
    """
    return (os.environ.get("SENTINEL_HOST") or "").strip() or None


async def _create_clients(specs: list[tuple[str, str, Optional[dict]]]) -> dict[str, WeilClient]:
    """Resolve all agent wallets concurrently (one RTT instead of N)."""
    sentinel_host = _sentinel_host_from_env()
    clients = await asyncio.gather(*[
        WeilClient.from_api_key(key, creds=creds, sentinel_host=sentinel_host)
        for _, key, creds in specs
    ])
    out = {}
    for (name, _, _), client in zip(specs, clients):
        _ = client.wallet_addr()  # fail loudly if wallet won't resolve
        out[name] = client
    return out


# ---------------------------------------------------------------- base

class BaseAgent:
    def __init__(self, name: str, limits: Optional[dict] = None,
                 system_prompt: str = ""):
        self.name = name
        self.limits = limits or {}
        self.system_prompt = system_prompt
        self.weil_client: Optional[WeilClient] = None

    def check_limits(self, action: str,
                     amount: Optional[float] = None,
                     action_key: Optional[str] = None) -> tuple[bool, Optional[str], Optional[float]]:
        """Pure Layer 2 limit check (no on-chain anchoring).

        Returns (authorized, refusal_reason, amount_approved).
        """
        authorized = True
        refusal_reason: Optional[str] = None
        amount_approved = amount

        # LAYER 2 — HARD ENFORCEMENT (substring-robust: no phrasing dodge).
        key = action_key or infer_limit_key(action)
        thresholds = self.limits.get("requires_human_above", {}) or {}
        if amount is not None and amount > 0 and thresholds:
            if key is None:
                # Unknown money-moving phrasing: enforce the strictest limit
                # so novel wording can never slip through.
                strictest = min(float(v) for v in thresholds.values())
                if amount > strictest:
                    authorized = False
                    amount_approved = 0.0
                    refusal_reason = (
                        f"attempted_{action} ${amount:.2f} exceeds {self.name} "
                        f"strictest financial limit of ${strictest:.2f}"
                    )
            else:
                max_allowed = thresholds.get(key)
                if max_allowed is not None and amount > float(max_allowed):
                    authorized = False
                    amount_approved = 0.0
                    refusal_reason = (
                        f"attempted_{action} ${amount:.2f} exceeds {self.name} "
                        f"max_{key} limit of ${float(max_allowed):.2f}"
                    )
        return authorized, refusal_reason, amount_approved


# ---------------------------------------------------------------- 4 agents

def create_intake_agent(client: WeilClient):
    class IntakeAgent(BaseAgent):
        @client.audit()
        async def run(self, ticket, policy: dict, customer: Optional[dict] = None):
            text = f"{ticket.subject} {ticket.body}"
            entities: list[str] = []

            for m in re.finditer(
                r"#\d+|order[_\s]?\d+|\$?\s?[\d,]+(?:\.\d{1,2})?",
                text,
                re.I,
            ):
                entities.append(
                    m.group(0).strip().replace(" ", "_")
                )

            lowered = text.lower()

            urgency = (
                0.9
                if any(
                    w in lowered
                    for w in ("urgent", "locked out", "entire team", "!!")
                )
                else 0.6
                if any(
                    w in lowered
                    for w in ("twice", "crash", "cannot", "refund")
                )
                else 0.3
            )

            summary = ticket.subject.strip()

            return {
                "entities": entities,
                "summary": summary,
                "urgency": urgency,
            }

    return IntakeAgent

def create_categorizer_agent(client: WeilClient):
    class CategorizerAgent(BaseAgent):
        @client.audit()
        async def run(self, ticket, policy: dict):
            
            data = await categorize_ticket(ticket, policy)
            return Category(**data)

    return CategorizerAgent

def create_router_agent(client: WeilClient):
    class RouterAgent(BaseAgent):
        @client.audit()
        async def run(self, ticket, category, policy: dict, db=None):

            data = await route_decision(ticket, category, policy, db)
            return Routing(**data)
    return RouterAgent


CANONICAL_ACTIONS = {
    "issue_refund": "issue_refund",
    "refund": "issue_refund",
    "refunds": "issue_refund",
    "grant_credit": "grant_credit",
    "credit": "grant_credit",
    "credits": "grant_credit",
    "waive_fee": "waive_fee",
    "fee_waiver": "waive_fee",
    "waive": "waive_fee",
    "waiver": "waive_fee",
    "send_response": "send_response",
    "respond": "send_response",
    "response": "send_response",
    "escalate": "send_response",
    "escalation": "send_response",
}

ACTION_OUTCOME = {
    "issue_refund": "refund_issued",
    "grant_credit": "credit_granted",
    "waive_fee": "fee_waived",
}

ACTION_VERB = {
    "issue_refund": "refund",
    "grant_credit": "credit",
    "waive_fee": "fee waiver",
}


def _canonical_action(raw: str) -> str:
    key = (raw or "").strip().lower()
    if not key:
        return "send_response"
    if key in CANONICAL_ACTIONS:
        return CANONICAL_ACTIONS[key]
    # Substring fallback: catches "refund request", "REFUND_APPROVED", etc.
    if "refund" in key:
        return "issue_refund"
    if "credit" in key:
        return "grant_credit"
    if "waiv" in key or "fee" in key:
        return "waive_fee"
    if "escalat" in key or key in ("send", "respond", "response"):
        return "send_response"
    return key  # unknown: preserved, still enforced via strictest-limit fallback

def create_resolver_agent(client: WeilClient):
    class ResolverAgent(BaseAgent):

        @client.audit()
        async def run(self, ticket, category, routing, policy: dict,
                sub_verdicts: Optional[list] = None, db=None):
            attempt = await resolve_ticket(ticket, category, routing, policy,
                                    db=db, sub_verdicts=sub_verdicts)
            action = _canonical_action(str(attempt.get("action_attempted", "send_response")))
            requested = _to_float_or_none(attempt.get("amount_requested"))

            # --- Sub-agent verdict processing ---
            # Fraud detector flagged: override to escalate, never move money
            fraud_verdicts = [v for v in (sub_verdicts or []) if v.get("agent") == "fraud_detector"]
            refund_verdicts = [v for v in (sub_verdicts or []) if v.get("agent") == "refund_validator"]
            tech_verdicts = [v for v in (sub_verdicts or []) if v.get("agent") == "tech_diagnostic"]

            fraud_blocked = any(v.get("verdict") == "flagged" for v in fraud_verdicts)
            refund_blocked = any(v.get("verdict") == "blocked" for v in refund_verdicts)

            # Fraud/validator blocks override ANY action — even a text-only reply
            # must not quietly close a flagged case.
            if fraud_blocked:
                # Fraud detected — escalate with context
                fraud_v = next(v for v in fraud_verdicts if v.get("verdict") == "flagged")
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=requested, amount_approved=None,
                    response=("I've escalated this to a human specialist for review."),
                    resolution_notes=f"FRAUD FLAG: {fraud_v.get('reasoning', '')}",
                    authorized=True, confidence=fraud_v.get("confidence", 0.8),
                    reasoning=f"fraud_detector flagged: {fraud_v.get('reasoning', '')}")

            if refund_blocked:
                # Refund validator blocked (e.g., outside refund window)
                refund_v = next(v for v in refund_verdicts if v.get("verdict") == "blocked")
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=requested, amount_approved=None,
                    response=("This request needs human review — I've escalated it with full context."),
                    resolution_notes=f"REFUND BLOCKED: {refund_v.get('reasoning', '')}",
                    authorized=True, confidence=refund_v.get("confidence", 0.8),
                    reasoning=f"refund_validator blocked: {refund_v.get('reasoning', '')}")

            # Cap the request at the DB-verified overcharge. The LLM often grabs
            # captured_total (all captures, e.g. $240) instead of the duplicate
            # figure (e.g. $120) — never refund more than was verifiably
            # overcharged, whatever the verdict status.
            capped_to_verified = False
            if refund_verdicts and requested and requested > 0:
                verified = max(
                    ((v.get("data") or {}).get("actual_overcharge") or 0)
                    for v in refund_verdicts
                )
                if verified > 0 and requested > verified:
                    requested = float(verified)
                    capped_to_verified = True

            # No amount from the LLM, but the validator verified a real overcharge
            # in the database: use the DB figure instead of guessing or escalating.
            if action != "send_response" and (requested is None or requested <= 0):
                for v in refund_verdicts:
                    actual = (v.get("data") or {}).get("actual_overcharge")
                    if actual and actual > 0:
                        requested = float(actual)
                        break

            # Tech diagnostic: include diagnosis in response
            tech_diagnosis = ""
            if tech_verdicts:
                tv = tech_verdicts[0]
                if tv.get("verdict") == "diagnosed":
                    tech_diagnosis = f"\n\nTechnical diagnosis: {tv.get('reasoning', '')}"

            # If the LLM says escalate, never move money — honor the escalation,
            # EXCEPT when the LLM mis-applied the limits: it chose a financial
            # action within limits on an auto-resolvable ticket but set
            # escalate=true out of limit-confusion ("below my limit, but I must
            # escalate due to policy"). The code owns the limits, so approve.
            if attempt.get("escalate") and action != "send_response":
                _within = False
                if requested is not None and requested > 0:
                    _authorized, _, _ = self.check_limits(action, float(requested))
                    _within = _authorized
                _auto = bool(routing.auto_resolvable)
                if not (_within and _auto):
                    return Resolution(
                        outcome="escalated_to_human", action_attempted=action,
                        amount_requested=requested, amount_approved=None,
                        response=(attempt.get("response") or
                                "This needs a human specialist — I've escalated it with full context.") + tech_diagnosis,
                        resolution_notes=f"LLM requested escalation; no funds moved (attempted {action} {requested}).",
                        authorized=True, confidence=attempt.get("confidence", 0.75),
                        reasoning=attempt.get("reasoning", "llm escalation honored; no funds moved"))
                # else: limit-confusion override — fall through to approve below.

            if action == "send_response" and attempt.get("escalate"):
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=None, amount_approved=None,
                    response=attempt["response"] + tech_diagnosis,
                    resolution_notes=attempt["resolution_notes"],
                    authorized=True, confidence=attempt.get("confidence", 0.75),
                    reasoning=attempt.get("reasoning", ""))

            if action == "send_response":
                return Resolution(
                    outcome="auto_resolved", action_attempted=action,
                    amount_requested=None, amount_approved=None,
                    response=attempt["response"] + tech_diagnosis,
                    resolution_notes=attempt["resolution_notes"],
                    authorized=True, confidence=attempt.get("confidence", 0.8),
                    reasoning=attempt.get("reasoning", ""))

            # Selective abstention: a consequential action the model itself
            # reports low confidence on is escalated, never auto-approved.
            # Threshold lives in policy (agent_limits.resolver.min_confidence).
            min_conf = float(self.limits.get("min_confidence", 0.5))
            llm_conf = _safe_float(attempt.get("confidence", 0.8), 0.8)
            if action != "send_response" and llm_conf < min_conf:
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=requested, amount_approved=None,
                    response=("This needs a human specialist — "
                              "I've escalated it with full context."),
                    resolution_notes=f"low model confidence ({llm_conf:.2f} < {min_conf:.2f}); escalated rather than acting",
                    authorized=True, confidence=llm_conf,
                    reasoning="abstained: low confidence on financial action")

            # Consequential financial action -> Layer 2 enforcement via check_limits().
            # A financial action without a usable amount is escalated, never guessed.
            if requested is None or requested <= 0:
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=None, amount_approved=None,
                    response=("I need a bit more detail to proceed safely — "
                            "I've escalated this to a human specialist."),
                    resolution_notes="financial action without a usable amount; escalated",
                    authorized=True, confidence=attempt.get("confidence", 0.6),
                    reasoning="no usable amount stated; escalated rather than guessing")
            # DB grounding: never move money for an account with no captured
            # charges — covers cases the validator never saw (skipped, bypassed,
            # or a non-billing ticket with a financial action).
            if db is not None and customer_captured_total(db, ticket.customer_id) <= 0:
                return Resolution(
                    outcome="escalated_to_human", action_attempted=action,
                    amount_requested=requested, amount_approved=None,
                    response=("I couldn't find any charges on your account to refund — "
                            "I've escalated this to a human specialist for review."),
                    resolution_notes="no captured charges on file; nothing to refund or waive",
                    authorized=True, confidence=0.85,
                    reasoning="customer has no captured payments; financial action refused without DB grounding")
            amount = float(requested)
            authorized, refusal_reason, _ = self.check_limits(action, amount)
            if not authorized:
                # Anchor the refusal explicitly: the @client.audit() wrapper
                # only logs inputs before run, never outcomes.
                await client.audit(json.dumps({
                    "event": "LAYER_2_REFUSAL",
                    "agent": "resolver",
                    "ticket_id": str(ticket.ticket_id),
                    "action": action,
                    "attempted_amount": amount,
                    "refusal_reason": refusal_reason,
                }))
                return Resolution(
                    outcome="blocked_by_limit", action_attempted=action,
                    amount_requested=amount, amount_approved=0.0,
                    response=(f"I can't {action.replace('_', ' ')} of ${amount:.2f} — "
                            f"that exceeds my authorization limit. I've escalated this "
                            f"to a human specialist who can review it."),
                    resolution_notes=f"BLOCKED: {refusal_reason}",
                    authorized=False, refusal_reason=refusal_reason,
                    confidence=attempt.get("confidence", 0.7),
                    reasoning=f"limit enforcement: {refusal_reason}")

            # Authorized: apply state change.
            outcome = ACTION_OUTCOME.get(action, "auto_resolved")
            verb = ACTION_VERB.get(action, "action")
            capped_note = (
                f" (capped to verified overcharge ${amount:.2f})"
                if capped_to_verified else ""
            )
            return Resolution(
                outcome=outcome, action_attempted=action,  # type: ignore[arg-type]
                amount_requested=amount, amount_approved=amount,
                response=f"Done — I've issued your {verb} of ${amount:.2f}. Let us know if you need anything else!",
                resolution_notes=f"{action} ${amount:.2f} within limits; applied.{capped_note}",
                authorized=True, confidence=attempt.get("confidence", 0.8),
                reasoning=attempt.get("reasoning", "within authorization limits"))
    return ResolverAgent


async def build_agents(policy, api_keys=None, credentials=None):
    api_keys = api_keys or {}

    def _spec(name):
        # Key errors surface here, before any network I/O.
        key = (
            api_keys.get(name)
            or os.environ.get(f"WEIL_API_KEY_{name.upper()}")
        )
        if not key:
            raise RuntimeError(
                f"No Weil API key for agent '{name}': pass api_keys['{name}'] "
                f"or set WEIL_API_KEY_{name.upper()} (or WEIL_API_KEY)."
            )
        # Per-agent dict wins, then explicit global, then WEIL_S3_*/AWS_* env.
        creds = (
            credentials if credentials is not None else _creds_from_env()
        )
        return (name, key, creds)

    names = ("intake", "categorizer", "router", "resolver")
    clients = await _create_clients([_spec(n) for n in names])

    intake = create_intake_agent(clients["intake"])(
        "intake", get_agent_limits(policy, "intake"), INTAKE_PROMPT)
    categorizer = create_categorizer_agent(clients["categorizer"])(
        "categorizer", get_agent_limits(policy, "categorizer"), CATEGORIZER_PROMPT)
    router = create_router_agent(clients["router"])(
        "router", get_agent_limits(policy, "router"), ROUTER_PROMPT)

    resolver_limits = get_agent_limits(policy, "resolver")
    resolver = create_resolver_agent(clients["resolver"])(
        "resolver", resolver_limits, build_resolver_prompt(
            resolver_limits.get("max_refund", 500.0),
            resolver_limits.get("max_credit", 200.0),
            resolver_limits.get("max_fee_waiver", 50.0),
        ))

    # Expose the signing client for lifecycle management (close_deps) and
    # explicit receipts. BaseAgent.weil_client exists for exactly this.
    for agent in (intake, categorizer, router, resolver):
        agent.weil_client = clients[agent.name]

    return {
        "intake": intake,
        "categorizer": categorizer,
        "router": router,
        "resolver": resolver,
    }