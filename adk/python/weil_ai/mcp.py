"""MCP integration for WeilChain — secured decorator and HTTP middleware.

Provides:
- ``secured(svc_name)``   — decorator factory that enforces on-chain access control
                            for FastMCP / MCP tool handlers.
- ``weil_middleware()``   — Starlette middleware that reads the ``weil-wallet-addr``
                            request header and stores it in a ContextVar.
- ``current_wallet_addr()`` — read the wallet address set by the middleware for the
                              current request.
"""

from __future__ import annotations

import contextvars
import functools
import json
from typing import Any, Callable, Type
import httpx
from weil_wallet.api.platform_api import PlatformApi
from weil_wallet.client import WeilClient
from weil_wallet.constants import SENTINEL_HOST
from weil_wallet.errors import WalletNotPermittedError
from weil_wallet.transaction import BaseTransaction, TransactionHeader
from weil_wallet.utils import current_time_millis
from .auth import verify_weil_signature

# One ContextVar object per process, but its *value* is per-asyncio-task.
# ASGI servers (uvicorn) run each HTTP request in a separate asyncio task, so
# each request gets its own isolated copy of the context. Calling .set() in one
# request's task never affects another concurrent request's task.
# Set by weil_middleware() on every inbound request; read by secured() and
# current_wallet_addr().
_weil_wallet_addr: contextvars.ContextVar[str] = contextvars.ContextVar(
    "weil_wallet_addr", default=""
)

# Maximum tolerated age of a request timestamp in seconds.
# The MCP streamable-HTTP transport reuses the same auth headers across all
# HTTP requests within a single session (initialize, list_tools, tool calls…),
# so per-message nonce tracking is not suitable here. The timestamp window
# is the replay-protection mechanism: captured headers cannot be replayed
# more than 5 minutes after they were issued.
_ALLOWED_TIMESTAMP_DRIFT: int = 300  # 5 minutes


def current_wallet_addr() -> str:
    """Return the wallet address injected by the current request's middleware.

    Returns an empty string when called outside a request context.
    """
    return _weil_wallet_addr.get()


def weil_middleware() -> Type[Any]:
    """Return a Starlette middleware class that verifies wallet ownership on every
    request and stores the verified address in a ContextVar.

    Expected request headers
    ------------------------
    X-Wallet-Address  : hex-encoded wallet address (SHA-256 of uncompressed pubkey)
    X-Signature       : hex-encoded 64-byte compact secp256k1 signature (r‖s)
    X-Message         : canonical JSON string that was signed
    X-Timestamp       : Unix timestamp (seconds) when the request was created

    The middleware rejects requests that:
    - are missing any of the four headers (401)
    - carry a timestamp older than 5 minutes (401 — prevents replay)
    - produce a recovered address that doesn't match X-Wallet-Address (401)

    On success the verified checksum address is stored in the ContextVar and
    is readable via ``current_wallet_addr()`` for the lifetime of the request.

    Usage::

        app = mcp.http_app(transport="streamable-http")
        app.add_middleware(weil_middleware())
    """
    try:
        from starlette.middleware.base import BaseHTTPMiddleware
        from starlette.responses import Response
    except ImportError as exc:
        raise ImportError(
            "starlette is required for weil_middleware(); "
            "install it with: pip install starlette"
        ) from exc

    class WeilHeaderMiddleware(BaseHTTPMiddleware):
        """Verifies wallet ownership via signature and stores the address in a ContextVar."""

        async def dispatch(self, request: Any, call_next: Any) -> Any:
            # Only verify auth for POST requests — those are the MCP tool calls.
            # GET /mcp establishes the SSE stream and never invokes a tool, so
            # it passes through without auth. weil_wallet_addr is only read
            # inside secured(), which only runs during POST-driven tool calls.
            if request.method != "POST":
                return await call_next(request)

            wallet_address = request.headers.get("X-Wallet-Address")
            signature_hex = request.headers.get("X-Signature")
            message = request.headers.get("X-Message")
            timestamp = request.headers.get("X-Timestamp")

            is_verified = verify_weil_signature(
                wallet_address, signature_hex, message, timestamp
            )

            if not is_verified:
                return Response("Wallet address verification failed", status_code=401)

            _weil_wallet_addr.set(wallet_address)
            return await call_next(request)

    return WeilHeaderMiddleware


def secured(svc_name: str) -> Callable:
    """Decorator factory for MCP/FastMCP tools that enforces on-chain access control.

    Resolves *svc_name* to a ContractId and calls ``key_has_purpose`` on it with
    the wallet address extracted from the request header before invoking the tool.
    Returns an MCP error response (``isError=True``) when the wallet is not permitted.

    Usage::

        @server.tool()
        @secured("engg::weil")
        async def my_tool(query: str) -> str:
            ...
    """

    def decorator(func: Callable) -> Callable:
        @functools.wraps(func)
        async def wrapper(*args: Any, **kwargs: Any) -> Any:
            import mcp.types as mcp_types

            def _permission_denied(wallet_addr: str) -> list:
                msg = str(WalletNotPermittedError(wallet_addr, svc_name))
                return [mcp_types.TextContent(type="text", text=msg, isError=True)]

            http_client = httpx.AsyncClient(
                base_url=SENTINEL_HOST,
                timeout=60.0,
            )
            applet_id = await WeilClient.get_applet_id_for_name(http_client, svc_name)
            wallet_addr = current_wallet_addr()

            from_addr = wallet_addr
            to_addr = from_addr
            weilpod_counter = applet_id.pod_counter()
            method_name = "key_has_purpose"
            method_args = json.dumps({"key": wallet_addr, "purpose": "Execution"})

            final_args = {
                "contract_address": applet_id,
                "contract_method": method_name,
                "contract_input_bytes": method_args,
                "should_hide_args": True,
            }

            nonce = int(current_time_millis())
            header = TransactionHeader(
                nonce=nonce,
                public_key="",
                from_addr=from_addr,
                to_addr=to_addr,
                weilpod_counter=weilpod_counter,
            )

            base_txn = BaseTransaction(header=header)
            payload = WeilClient._build_submit_payload("", base_txn, final_args)

            resp = await PlatformApi.submit_transaction(
                payload,
                http_client,
                is_non_blocking=False,
            )

            txn_result = json.loads(resp.txn_result)

            if "Ok" not in txn_result:
                return _permission_denied(wallet_addr)

            flag = txn_result["Ok"]

            if flag == "false":
                flag = False
            elif flag == "true":
                flag = True
            else:
                return _permission_denied(wallet_addr)

            if not flag:
                return _permission_denied(wallet_addr)

            return await func(*args, **kwargs)

        # functools.wraps copies the original function's return annotation (e.g. -> str).
        # FastMCP uses that to validate the return value and would reject the TextContent
        # error list with "Output validation error: ... is not of type 'string'".
        # Clearing the annotation lets FastMCP pass the value through as-is.
        wrapper.__annotations__.pop("return", None)

        return wrapper

    return decorator
