"""MCP integration for WeilChain — secured decorators and HTTP middleware.

Provides:
- ``secured_user(rule)``  — decorator factory that allows Execution or
                            Management wallet purpose.
- ``secured_admin(rule)`` — decorator factory that allows Management wallet
                            purpose only.

  *rule* is one scope (``"engg.weil"``, ``"weil"``) or a boolean rule over
  several, e.g. ``"(engg.weil | legal.weil) & !contractors.weil"`` — the same
  language and evaluation as the Rust ``#[secured_user]``/``#[secured_admin]``
  macros. See ``weil_ai.secured_expr``.
- ``weil_middleware()``   — Starlette middleware that verifies signed Weil auth
                            headers and stores the caller in a ContextVar.
- ``current_wallet_addr()`` — read the wallet address active in the current
                              request or tool context.
"""

from __future__ import annotations

import contextvars
import concurrent.futures
import asyncio
import functools
import inspect
import logging
import threading
import json
from typing import Any, Callable, Type
import httpx
from weil_wallet.api.platform_api import PlatformApi
from weil_wallet.client import WeilClient
from weil_wallet.constants import IDENTITY_APPLET_SVC_PREFIX
from weil_wallet.contract import ContractId
from weil_wallet.constants import SENTINEL_HOST
from weil_wallet.errors import WalletNotPermittedError
from weil_wallet.transaction import BaseTransaction, TransactionHeader
from weil_wallet.utils import current_time_millis
from .auth import full_address_from_headers, verify_weil_signature
from .secured_expr import (
    Outcome,
    SecuredExprError,
    evaluate_root_first,
    parse_secured_expr,
    secured_orgs,
)

_logger = logging.getLogger(__name__)

# One ContextVar object per process, but its *value* is per-asyncio-task.
# ASGI servers (uvicorn) run each HTTP request in a separate asyncio task, so
# each request gets its own isolated copy of the context.
# Async tasks inherit a context when they are created; setting
# a value in one task does not mutate another task's context.
# Middleware and agent integrations set these values around secured work.
_weil_wallet_addr: contextvars.ContextVar[str] = contextvars.ContextVar(
    "weil_wallet_addr", default=""
)
_weil_context_mode: contextvars.ContextVar[str] = contextvars.ContextVar(
    "weil_context_mode", default=""
)
_weil_sentinel_host: contextvars.ContextVar[str] = contextvars.ContextVar(
    "weil_sentinel_host", default=SENTINEL_HOST
)
_weil_verify_tls: contextvars.ContextVar[bool] = contextvars.ContextVar(
    "weil_verify_tls", default=True
)

# Maximum tolerated age of a request timestamp in seconds.
# The MCP streamable-HTTP transport reuses the same auth headers across all
# HTTP requests within a single session (initialize, list_tools, tool calls…),
# so per-message nonce tracking is not suitable here. The timestamp window
# is the replay-protection mechanism: captured headers cannot be replayed
# more than 5 minutes after they were issued.
_ALLOWED_TIMESTAMP_DRIFT: int = 300  # 5 minutes
_SECURED_CHECK_TOPIC = "security"
# Metadata field in which audit() returns the entry in the tool result.
AUDIT_FIELD = "audit_payload"
# Set on @secured wrappers so framework audit hooks can tell a guarded tool
# apart and record its arguments and response.
_SECURED_ATTR = "__weil_secured__"


class SecuredCheckError(RuntimeError):
    """Raised when a secured rule's outcome could not be decided.

    Some check the rule depended on failed (identity applet missing, a
    ``key_has_purpose`` call errored), and the rest of the rule could not
    decide the result without it. Callers treat this as "not permitted";
    the message lists what failed.
    """

    def __init__(self, rule: str, failures: list[str]) -> None:
        self.rule = rule
        self.failures = list(failures)
        detail = "; ".join(self.failures) or "unknown failure"
        super().__init__(f"secured check for {rule!r} could not be completed: {detail}")


def _publisher_name(org: str) -> str:
    return f"publisher::{org}"


def current_wallet_addr() -> str:
    """Return the wallet address active in the current request/tool context.

    Middleware and agent integrations set this value. 
    Returns an empty string when called outside a secured request or tool execution.
    """
    return _weil_wallet_addr.get()


def set_current_wallet_addr(
    wallet_addr: str,
    *,
    mode: str = "",
    sentinel_host: str = SENTINEL_HOST,
    verify: bool = True,
) -> tuple[Any, Any, Any, Any]:
    """Set the wallet and request settings used by secured checks.

    Keep the returned tokens and pass them to ``reset_current_wallet_addr``
    when this request or tool execution finishes.
    """
    wallet_token = _weil_wallet_addr.set(wallet_addr)
    mode_token = _weil_context_mode.set(mode)
    sentinel_host_token = _weil_sentinel_host.set(sentinel_host)
    verify_token = _weil_verify_tls.set(verify)
    return wallet_token, mode_token, sentinel_host_token, verify_token


def reset_current_wallet_addr(tokens: tuple[Any, Any, Any, Any]) -> None:
    """Reset tokens returned by set_current_wallet_addr()."""
    wallet_token, mode_token, sentinel_host_token, verify_token = tokens
    _weil_wallet_addr.reset(wallet_token)
    _weil_context_mode.reset(mode_token)
    _weil_sentinel_host.reset(sentinel_host_token)
    _weil_verify_tls.reset(verify_token)


def weil_middleware(
    *,
    sentinel_host: str = SENTINEL_HOST,
    verify: bool = True,
) -> Type[Any]:
    """Return middleware that verifies signed POST requests and stores the wallet.

    Expected request headers
    ------------------------
    X-Wallet-Address  : the caller's full account address (72-char hex)
    X-Signature       : hex-encoded 64-byte compact secp256k1 signature (r‖s)
    X-Message         : canonical JSON string that was signed
    X-Timestamp       : Unix timestamp (seconds) when the request was created

    The middleware rejects requests that:
    - are missing any of the four headers (401)
    - carry a timestamp outside the allowed 5-minute clock-skew window (401)
    - produce a recovered signing-key address that doesn't match the trailing
      64 chars of X-Wallet-Address (401)
    - carry an X-Wallet-Address that is not a 72-char hex string (401)

    On success the verified address is stored in the ContextVar and is readable
    via ``current_wallet_addr()`` for the lifetime of the request. ``sentinel_host``
    and ``verify`` are likewise stored per-request so @secured decorators resolve
    applets against the same environment the server was configured for, instead
    of always falling back to the ``SENTINEL_HOST`` package default.

    Usage::

        app = mcp.http_app(transport="streamable-http")
        app.add_middleware(weil_middleware(sentinel_host=os.environ["SENTINEL_HOST"]))
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
            # Streamable HTTP uses POST for MCP operations, including tool calls;
            # GET opens the SSE stream and does not invoke secured tools.
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

            # Identity lookups key on the canonical address (pod counter + key
            # hash), which the client signs. Older clients omit it; fall back
            # to the key-derived form.
            caller_addr = (
                full_address_from_headers(request.headers) or wallet_address
            )

            tokens = set_current_wallet_addr(
                caller_addr, mode="mcp", sentinel_host=sentinel_host, verify=verify
            )
            try:
                return await call_next(request)
            finally:
                reset_current_wallet_addr(tokens)

    return WeilHeaderMiddleware


# NOTE: the MCP-only `secured()` that lived here has been folded into
# `_secured()` below, which checks purposes on the identity applet while also
# supporting Management-only scopes (`secured_admin`) and raising for
# non-MCP callers such as the ADK tool gate. Two `def secured(...)` in one
# module meant the second shadowed this one, so this path never ran.

async def _publish_secured_check(
    http_client: httpx.AsyncClient,
    wallet_addr: str,
    publisher_svc_name: str,
    svc_name: str,
    passed: bool,
) -> None:
    """Publish the secured-check verdict without changing auth behavior."""
    try:
        publisher_id = await WeilClient.get_applet_id_for_name(
            http_client,
            publisher_svc_name,
            wallet_address=wallet_addr,
        )
        verdict = "passed" if passed else "failed"
        method_args = json.dumps(
            {
                "topic": _SECURED_CHECK_TOPIC,
                "payload": f"{wallet_addr} {verdict} @secured check for {svc_name}",
            }
        )
        await _call_secured_contract(
            http_client,
            publisher_id,
            wallet_addr,
            "publish",
            method_args,
            is_non_blocking=True,
        )
    except Exception:
        logging.getLogger(__name__).exception("Failed to publish secured-check event")


async def _secured_key_has_purpose(
    http_client: httpx.AsyncClient,
    applet_id: ContractId,
    wallet_addr: str,
    key: str,
    purpose: str,
) -> bool:
    """Return whether *key* holds *purpose* on the given identity applet."""
    value = await _call_secured_contract(
        http_client,
        applet_id,
        wallet_addr,
        "key_has_purpose",
        json.dumps({"key": key, "purpose": purpose}),
    )
    return value is True or value == "true"


async def wallet_has_purpose(
    wallet_addr: str,
    svc_name: str,
    *,
    purposes: tuple[str, ...],
    sentinel_host: str = SENTINEL_HOST,
    verify: bool = True,
    http_client: httpx.AsyncClient | None = None,
) -> bool:
    """Return whether *wallet_addr* satisfies the secured rule *svc_name*.

    *svc_name* is one scope (``"engg.weil"``) or a rule over several
    (``"infrastructure.weil & !finance.weil"``); each key must hold one of
    *purposes*. Raises ``SecuredCheckError`` when the outcome could not be
    decided because a check the rule depended on failed.
    """
    return (
        await _granting_purpose(
            wallet_addr,
            svc_name,
            purposes=purposes,
            sentinel_host=sentinel_host,
            verify=verify,
            http_client=http_client,
        )
        is not None
    )


async def _granting_purpose(
    wallet_addr: str,
    svc_name: str,
    *,
    purposes: tuple[str, ...],
    sentinel_host: str = SENTINEL_HOST,
    verify: bool = True,
    http_client: httpx.AsyncClient | None = None,
) -> str | None:
    """Return the purpose that grants *wallet_addr* access under *svc_name*, or None.

    *svc_name* is a secured rule — one scope or a boolean rule over several —
    evaluated "root access first", like the Rust macros: the caller's
    org-level key (the bare wallet address) is checked once for every org the
    rule names, then each ``"subgroup.org"`` condition passes on that
    org-level key or on ``"<subgroup>_<addr>"``.

    Each key is probed for *purposes* in the order given; the purpose
    returned is the first one that matched while evaluating the rule. A rule
    satisfied without any key matching (only ``!`` conditions) reports the
    first purpose listed.

    One client, and one identity-applet lookup per org, serve every probe.
    Pass *http_client* to reuse an already-open client.

    Raises:
        SecuredExprError: *svc_name* is not a valid rule.
        SecuredCheckError: a check the rule depended on failed, so the
            outcome is unknown. A failure on a branch the rule did not need
            (e.g. the losing side of an already-true ``|``) does not raise.
    """
    expr = parse_secured_expr(svc_name)

    async def check(client: httpx.AsyncClient) -> str | None:
        applet_ids: dict[str, ContractId | None] = {}
        matched: list[str] = []
        failures: list[str] = []

        async def applet_id_for(org: str) -> ContractId | None:
            if org not in applet_ids:
                applet_name = f"{IDENTITY_APPLET_SVC_PREFIX}{org}"
                try:
                    applet_ids[org] = await WeilClient.get_applet_id_for_name(
                        client, applet_name, wallet_address=wallet_addr
                    )
                except Exception as exc:
                    failures.append(f"{applet_name} did not resolve: {exc}")
                    applet_ids[org] = None
            return applet_ids[org]

        async def check_key(org: str, key: str) -> Outcome:
            """Does *key* hold one of *purposes* on *org*? None if unknown."""
            applet_id = await applet_id_for(org)
            if applet_id is None:
                return None
            unknown = False
            for purpose in purposes:
                try:
                    if await _secured_key_has_purpose(
                        client, applet_id, wallet_addr, key, purpose
                    ):
                        matched.append(purpose)
                        return True
                except Exception as exc:
                    failures.append(f"key_has_purpose({key!r}, {purpose}) on {org!r} failed: {exc}")
                    unknown = True
            return None if unknown else False

        outcome = await evaluate_root_first(expr, check_key, wallet_addr)
        if outcome is True:
            return matched[0] if matched else (purposes[0] if purposes else None)
        if outcome is None:
            for failure in failures:
                _logger.warning("secured %r: %s", svc_name, failure)
            raise SecuredCheckError(svc_name, failures)
        return None

    if http_client is not None:
        return await check(http_client)
    async with httpx.AsyncClient(
        base_url=sentinel_host.rstrip("/"),
        verify=verify,
        timeout=60.0,
    ) as client:
        return await check(client)

async def _call_secured_contract(
    http_client: httpx.AsyncClient,
    contract_address: Any,
    wallet_addr: str,
    method_name: str,
    method_args: str,
    *,
    is_non_blocking: bool = False,
) -> Any:
    """Submit a read-only contract call and return its unwrapped result.

    The transaction carries no public key and no signature: ``key_has_purpose``
    changes no state, so the chain does not require authorization to ask it.
    ``from_addr`` names the wallet being asked about, not one proving anything —
    that proof happened earlier, in the signature verification.
    """
    from_addr = wallet_addr
    to_addr = from_addr
    weilpod_counter = contract_address.pod_counter()

    final_args = {
        "contract_address": contract_address,
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
        is_non_blocking=is_non_blocking,
    )

    return _unwrap_secured_result(resp.txn_result)


def _unwrap_secured_result(txn_result: str) -> Any:
    """Unwrap an applet's {"Ok": ...} envelope, returning None on {"Err": ...}."""
    try:
        value = json.loads(txn_result)
    except (TypeError, ValueError):
        return None

    if not isinstance(value, dict) or "Ok" not in value:
        return None

    ok_value = value["Ok"]
    if isinstance(ok_value, str):
        try:
            return json.loads(ok_value)
        except ValueError:
            return ok_value
    return ok_value


def secured_user(svc_name: str) -> Callable:
    """Decorator factory for user tools that enforce on-chain access control.

    *svc_name* is a secured rule: one scope (``"engg.weil"``, or ``"weil"``
    for org-level) or a boolean rule over several using ``!`` (not), ``&``
    (and), ``|`` (or) and parentheses, with the same meaning as the Rust
    ``#[secured_user]`` macro. Before invoking the tool, the verified caller
    wallet is checked with ``key_has_purpose`` on each org's identity applet;
    keys holding either the Execution or Management purpose count.

    Org-level access is checked first: for every org the rule names, holding
    the org-level key (the bare wallet address) satisfies every condition in
    that org — including one under ``!``. Otherwise ``"engg.weil"`` needs the
    ``engg_<addr>`` key. A malformed rule raises ``SecuredExprError`` when the
    decorator is applied.

    The caller wallet can come from Weil MCP middleware or Weil-secured ADK
    A2A middleware. MCP/FastMCP tools keep the existing
    ``TextContent(isError=True)`` response behavior. Other integrations,
    including Google ADK tools, raise ``WalletNotPermittedError`` when the
    caller is not permitted.

    Usage::

        @server.tool()
        @secured_user("engg.weil")
        async def my_tool(query: str) -> str:
            ...

        @server.tool()
        @secured_user("(engg.weil | legal.weil) & !contractors.weil")
        async def review(doc_id: str) -> str:
            ...
    """
    return _secured(svc_name, allow_execution=True)


def secured_admin(svc_name: str) -> Callable:
    """Decorator factory for admin tools that require Management purpose.

    Takes the same rules as ``secured_user``; every key checked must hold
    the Management purpose.
    """
    return _secured(svc_name, allow_execution=False)


def secured(svc_name: str) -> Callable:
    """Compatibility alias for secured_user()."""
    return secured_user(svc_name)


def _secured(svc_name: str, *, allow_execution: bool) -> Callable:
    """Build a secured decorator for the requested access level."""

    # Decorators have always required dot syntax; A2A service names passed
    # straight to wallet_has_purpose may still use "::" as before.
    if "::" in svc_name:
        raise SecuredExprError(
            'secured scope must use dot syntax, e.g. @secured_user("subgroup.org")'
        )
    # Parse now, so a malformed rule fails when the tool is defined.
    publisher_names = [_publisher_name(org) for org in secured_orgs(parse_secured_expr(svc_name))]
    secured_info = {
        "service": svc_name,
        "access": "user" if allow_execution else "admin",
    }

    def decorator(func: Callable) -> Callable:
        async def _check_permission() -> Any:
            """Ask the identity applet whether the caller holds the purpose."""
            wallet_addr = current_wallet_addr()
            if not wallet_addr:
                return _permission_denied(wallet_addr or "", svc_name)
            
            async with httpx.AsyncClient(
                base_url=_weil_sentinel_host.get().rstrip("/"),
                verify=_weil_verify_tls.get(),
                timeout=60.0,
            ) as http_client:
                try:
                    permitted = await wallet_has_purpose(
                        wallet_addr,
                        svc_name,
                        purposes=("Management", "Execution") if allow_execution else ("Management",),
                        http_client=http_client,
                    )
                except SecuredCheckError:
                    # Undecidable (a check the rule needed failed): fail closed.
                    permitted = False
                # One verdict per org the rule names, to that org's publisher.
                for publisher_name in publisher_names:
                    await _publish_secured_check(
                        http_client,
                        wallet_addr,
                        publisher_name,
                        svc_name,
                        permitted,
                    )
                if not permitted:
                    return _permission_denied(wallet_addr, svc_name)
                return None

        if asyncio.iscoroutinefunction(func):
            @functools.wraps(func)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                denied = await _check_permission()
                if denied is not None:
                    return denied
                return await func(*args, **kwargs)

            # functools.wraps copies the original function's return annotation (e.g. -> str).
            # FastMCP uses that to validate the return value and would reject the TextContent
            # error list with "Output validation error: ... is not of type 'string'".
            # Clearing the annotation lets FastMCP pass the value through as-is.
            async_wrapper.__annotations__.pop("return", None)
            setattr(async_wrapper, _SECURED_ATTR, secured_info)
            return async_wrapper

        @functools.wraps(func)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            denied = _run_sync(_check_permission())
            if denied is not None:
                return denied
            return func(*args, **kwargs)

        sync_wrapper.__annotations__.pop("return", None)
        setattr(sync_wrapper, _SECURED_ATTR, secured_info)
        return sync_wrapper

    return decorator



def audit(name: str = "") -> Callable[[Callable], Callable]:
    """Decorator that returns an audit entry with the tool result.

    The server builds the entry — verified caller, tool and arguments —
    and attaches it to the result's metadata under ``AUDIT_FIELD``
    (``audit_payload``). The calling agent reads it and submits it from its own
    wallet with ``agent.audit(entry)``. Stack it below ``@secured`` so only
    authorized calls produce an entry.

    Usage::

        @mcp.tool()
        @secured("infrastructure.weil")
        @audit()
        async def my_tool(query: str) -> str: ...
    """

    def decorator(func: Callable) -> Callable:
        sig = inspect.signature(func)
        tool_name = name or func.__name__

        def _wrap(result: Any, args: tuple, kwargs: dict) -> Any:
            from fastmcp.tools.tool import ToolResult

            bound = sig.bind(*args, **kwargs)
            entry = json.dumps(
                {
                    "caller": current_wallet_addr(),
                    "tool": tool_name,
                    "args": {
                        k: v
                        for k, v in bound.arguments.items()
                        if k not in ("self", "cls")
                    },
                },
                default=repr,
            )
            if not isinstance(result, ToolResult):
                result = ToolResult(content=result)
            result.meta = {**(result.meta or {}), AUDIT_FIELD: entry}
            return result

        if asyncio.iscoroutinefunction(func):

            @functools.wraps(func)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                return _wrap(await func(*args, **kwargs), args, kwargs)

            # Same reason as @secured: the result is no longer a plain str.
            async_wrapper.__annotations__.pop("return", None)
            return async_wrapper

        @functools.wraps(func)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            return _wrap(func(*args, **kwargs), args, kwargs)

        sync_wrapper.__annotations__.pop("return", None)
        return sync_wrapper

    return decorator


audit_payload = audit


def _secured_tool_info(tool: Any) -> "dict[str, str] | None":
    """Return @secured metadata for a tool, or None if it is not guarded.

    Looks through framework tool wrappers (ADK ``FunctionTool.func``,
    LangChain ``StructuredTool.func``/``coroutine``) to the decorated function.
    """
    for candidate in (
        tool,
        getattr(tool, "func", None),
        getattr(tool, "coroutine", None),
    ):
        info = getattr(candidate, _SECURED_ATTR, None)
        if isinstance(info, dict):
            return info
    return None


def _permission_denied(wallet_addr: str, svc_name: str) -> Any:
    exc = WalletNotPermittedError(wallet_addr, svc_name)
    if _weil_context_mode.get() == "mcp":
        import mcp.types as mcp_types

        return [mcp_types.TextContent(type="text", text=str(exc), isError=True)]
    raise exc


_sync_loop: "asyncio.AbstractEventLoop | None" = None
_sync_loop_lock = threading.Lock()


def _get_sync_loop() -> asyncio.AbstractEventLoop:
    """Return the process-wide loop that sync callers bridge onto.

    WeilClient keeps a pooled httpx.AsyncClient whose connections are bound to
    the loop that opened them. Running every sync call on one long-lived loop
    (instead of a fresh asyncio.run per call) keeps those connections usable.
    """
    global _sync_loop
    with _sync_loop_lock:
        if _sync_loop is None or _sync_loop.is_closed():
            loop = asyncio.new_event_loop()
            threading.Thread(
                target=loop.run_forever, name="weil-sync-loop", daemon=True
            ).start()
            _sync_loop = loop
        return _sync_loop


def _run_sync(coro: Any) -> Any:
    loop = _get_sync_loop()
    try:
        running = asyncio.get_running_loop()
    except RuntimeError:
        running = None

    if running is loop:
        # Called from code already on the bridge loop: blocking on it would
        # deadlock, so fall back to a throwaway loop in a worker thread.
        ctx = contextvars.copy_context()
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            return pool.submit(ctx.run, asyncio.run, coro).result()

    # run_coroutine_threadsafe carries the caller's contextvars into the task.
    return asyncio.run_coroutine_threadsafe(coro, loop).result()
