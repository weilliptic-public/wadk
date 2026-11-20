"""weil_ai — MCP and AI integrations for WeilChain."""

from .agents import WeilAgent, agent_http_app, weil_agent
from .agents import weil_agent as agent
from .auth import build_auth_headers, verify_weil_signature
from .mcp import (
    SecuredCheckError,
    audit,
    current_wallet_addr,
    audit_payload,
    secured,
    secured_admin,
    secured_user,
    weil_middleware,
)
from .secured_expr import SecuredExprError
from weil_wallet.pensieve import PENSIEVE_APPLET_SVC_NAME, Pensieve

from .memory import (
    make_adk_pensieve_tools,
    make_langchain_pensieve_tools,
    make_strands_pensieve_tools,
)

__all__ = [
    # agents
    "WeilAgent",
    "weil_agent",
    "agent",
    "agent_http_app",
    # auth
    "build_auth_headers",
    "verify_weil_signature",
    # mcp
    "audit",
    "audit_payload",
    "secured",
    "secured_user",
    "secured_admin",
    "SecuredCheckError",
    "SecuredExprError",
    "weil_middleware",
    "current_wallet_addr",
    # pensieve
    "Pensieve",
    "PENSIEVE_APPLET_SVC_NAME",
    "make_adk_pensieve_tools",
    "make_langchain_pensieve_tools",
    "make_strands_pensieve_tools",
]
