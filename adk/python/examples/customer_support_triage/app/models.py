from __future__ import annotations
from typing import Literal, Optional, TypedDict
from pydantic import BaseModel, Field


class Ticket(BaseModel):
    ticket_id: str
    customer_id: str
    channel: Literal["email", "chat", "phone", "social"]
    subject: str
    body: str
    account_tier: Literal["free", "pro", "enterprise"] = "free"


class Category(BaseModel):
    primary: Literal["billing", "technical", "feature_request", "account", "bug", "how_to", "other"]
    sub_category: str = "general"
    sentiment: Literal["positive", "neutral", "negative", "angry"] = "neutral"
    confidence: float = 0.0
    reasoning: str = ""


class Routing(BaseModel):
    team: Literal["billing_team", "tech_support", "product_team", "account_management", "engineering"]
    priority: Literal["low", "medium", "high", "critical"]
    auto_resolvable: bool = False
    escalation_reason: Optional[str] = None
    reasoning: str = ""


class Resolution(BaseModel):
    outcome: Literal[
        "auto_resolved",
        "escalated_to_human",
        "needs_more_info",
        "refund_issued",
        "credit_granted",
        "fee_waived",
        "blocked_by_limit",
    ]
    action_attempted: str = "send_response"
    amount_requested: Optional[float] = None
    amount_approved: Optional[float] = None
    response: str = ""
    resolution_notes: str = ""
    authorized: bool = True
    refusal_reason: Optional[str] = None
    confidence: float = 0.0
    reasoning: str = ""


class SubAgentVerdict(BaseModel):
    agent: str  # "refund_validator", "fraud_detector", "tech_diagnostic"
    verdict: str  # "approved", "flagged", "blocked", "diagnosed", "clear"
    confidence: float = 0.0
    reasoning: str = ""
    data: dict = Field(default_factory=dict)  # agent-specific structured data


class TriageState(TypedDict, total=False):
    ticket: Ticket
    category: Optional[Category]
    routing: Optional[Routing]
    resolution: Optional[Resolution]
    halted: bool
    halt_reason: Optional[str]
    intake_data: Optional[dict]  # {entities, summary, urgency} from intake
    sub_agent_verdicts: list[dict]  # [{agent, verdict, confidence, reasoning, data}]
    spawned_agents: list[str]       # which sub-agents were dispatched
