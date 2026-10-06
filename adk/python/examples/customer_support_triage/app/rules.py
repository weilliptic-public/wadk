"""Policy engine helpers + limit loader.

policy_rules.json is configuration only — it defines limits but enforces
nothing by itself. Enforcement lives in app/agents.py (BaseAgent.check_limits).
"""
from __future__ import annotations

import json
import re
from functools import lru_cache
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
POLICY_PATH = PROJECT_ROOT / "policy_rules.json"


@lru_cache(maxsize=1)
def load_policy(path: str | Path | None = None) -> dict:
    p = Path(path) if path else POLICY_PATH
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def get_team_for_category(policy: dict, primary: str) -> str:
    return policy.get("team_routing", {}).get(primary, "tech_support")


def check_escalation_triggers(policy: dict, text: str) -> list[str]:
    lowered = text.lower()
    return [t for t in policy.get("escalation_triggers", []) if t.lower() in lowered]


def get_agent_limits(policy: dict, agent_name: str) -> dict:
    return policy.get("agent_limits", {}).get(agent_name, {})


# Map consequential financial actions -> limit keys in policy_rules.json
ACTION_TO_LIMIT_KEY = {
    "issue_refund": "refund",
    "refund": "refund",
    "grant_credit": "credit",
    "credit": "credit",
    "waive_fee": "fee_waiver",
    "fee_waiver": "fee_waiver",
}


def infer_limit_key(action: str) -> str | None:
    """Robust substring inference so prompt-injected LLM phrasing
    (e.g. 'refund request', 'REFUND_APPROVED', 'waive all fees') can never
    dodge Layer 2 enforcement via creative wording."""
    a = (action or "").strip().lower()
    if not a:
        return None
    if a in ACTION_TO_LIMIT_KEY:
        return ACTION_TO_LIMIT_KEY[a]
    if "refund" in a:
        return "refund"
    if "credit" in a:
        return "credit"
    if "waiv" in a or "fee" in a:
        return "fee_waiver"
    return None


_AMOUNT_RE = re.compile(r"\$\s?([\d,]+(?:\.\d{1,2})?)")


def requested_amount(ticket) -> float | None:
    """Largest dollar figure in the ticket's subject or body, or None.

    The demand is the biggest number on the ticket: one that opens with a
    "$400 overcharge" and then insists on "$4,000" is asking for $4,000.
    """
    text = f"{getattr(ticket, 'subject', '')} {getattr(ticket, 'body', '')}"
    found = [float(m.replace(",", "")) for m in _AMOUNT_RE.findall(text)]
    return max(found) if found else None
