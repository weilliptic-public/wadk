"""weil_ai — MCP and AI integrations for WeilChain."""

from .agents import WeilAgent, weil_agent
from .agents import weil_agent as agent
from .auth import build_auth_headers, verify_weil_signature
from .mcp import current_wallet_addr, secured, weil_middleware

__all__ = [
    # agents
    "WeilAgent",
    "weil_agent",
    "agent",
    # auth
    "build_auth_headers",
    "verify_weil_signature",
    # mcp
    "secured",
    "weil_middleware",
    "current_wallet_addr",
]
