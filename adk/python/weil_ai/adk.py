"""Google ADK integration for Weil agent identity, A2A signing, and audit."""

from __future__ import annotations

import asyncio
import inspect
import json
import time
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from weil_wallet.errors import WalletNotPermittedError

from .auth import (
    MAX_TIMESTAMP_AGE_SECONDS,
    _body_sha256,
    full_address_from_headers,
    verify_weil_request,
)
from .mcp import (
    _secured_tool_info,
    current_wallet_addr,
    reset_current_wallet_addr,
    set_current_wallet_addr,
    _granting_purpose,
)
from .memory import append_unique_tools, make_adk_pensieve_tools
from .tracer import (
    arecord_tool_call,
    begin_trace,
    current_parent_id,
    current_trace_id,
    extract_prompt,
    finish_trace,
    reset_trace_parent,
    set_trace_parent,
)


_SECURE_A2A_ATTR = "__weil_secure_a2a__"

# Purposes accepted by the inbound A2A service gate.
_EXECUTION_PURPOSE = "Execution"
_MANAGEMENT_PURPOSE = "Management"

# Auth headers build_auth_headers() sets; all four are required to verify.
_REQUIRED_AUTH_HEADERS = ("x-wallet-address", "x-signature", "x-message", "x-timestamp")


def setup_secure_a2a(
    agent: Any,
    *,
    service: Optional[str] = None,
    audit: bool = True,
    verify_signature: bool = True,
    pensieve: bool = True,
) -> Any:
    """Patch an ADK agent tree with Weil A2A signing and audit hooks.

    Serving agents use *service* as the permission gate on inbound A2A calls;
    calling/root agents usually omit it. Remote sub-agents are patched with
    outbound interceptors that sign each A2A message with this agent's wallet.

    When *service* is set, the inbound caller is checked at the middleware.
    Inside the tree, secured tools authorize as the acting agent, and audit
    records use that agent's wallet; ``caller_wallet`` records who invoked it.
    """

    weil_agent = _as_weil_agent(agent)
    inner = _inner(agent)
    config = {
        "service": service,
        "audit": audit,
        "verify_signature": verify_signature,
        "pensieve": pensieve,
    }
    setattr(inner, _SECURE_A2A_ATTR, config)
    if pensieve:
        _install_pensieve_tools(weil_agent, inner)
    _compose_agent_callbacks(weil_agent, inner, service=service, audit=audit)
    _secure_sub_agents(weil_agent, inner, audit=audit)

    return agent


def _secure_sub_agents(parent: Any, agent: Any, *, audit: bool) -> None:
    """Patch an agent's sub-agents recursively.

    Remote A2A calls are signed by *parent*, the nearest enclosing agent with
    a wallet. Local sub-agents use their own proxy's wallet when available;
    otherwise they inherit *parent*'s wallet for tool authorization and audits.
    """
    for sub_agent in _sub_agents(agent):
        # ADK stores the unwrapped agent; recover its proxy (and wallet).
        sub_agent = getattr(sub_agent, "__weil_agent__", sub_agent)
        sub = _inner(sub_agent)
        if _looks_like_remote_a2a_agent(sub):
            _patch_remote_agent(owner=parent, remote_agent=sub, audit=audit)
            continue
        # The agent whose wallet signs this sub-agent's audit records and
        # authorizes its tools: its own if it has one, else its enclosing agent's.
        acting = sub_agent if _has_own_identity(sub_agent) else parent
        # A nested local invocation is not an inbound HTTP request, so pass
        # the enclosing agent explicitly for caller attribution in its audits.
        caller_parent = parent if acting is not parent else None

        if not _secure_config(sub):
            _compose_agent_callbacks(
                acting, sub, service=None, audit=audit, parent=caller_parent
            )
        _secure_sub_agents(acting, sub, audit=audit)


def secure_a2a_app(
    app: Any,
    agent: Any,
    *,
    service: Optional[str] = None,
    audit: bool = True,
) -> Any:
    """Add inbound Weil verification middleware to a Starlette A2A app."""

    weil_agent = _as_weil_agent(agent)
    config = _secure_config(_inner(agent))
    resolved_service = service or config.get("service")
    verify_signature = config.get("verify_signature", True)

    app.add_middleware(
        WeilA2AMiddleware,
        agent=weil_agent,
        service=resolved_service,
        audit=audit,
        verify_signature=verify_signature,
    )
    return app


def to_secure_a2a(agent: Any, **kwargs: Any) -> Any:
    """Convert a Weil-wrapped ADK agent to A2A and secure the resulting app."""

    try:
        from google.adk.a2a.utils.agent_to_a2a import to_a2a
    except ImportError as exc:
        raise ImportError(
            "google-adk is required for to_secure_a2a(); install google-adk first"
        ) from exc

    if not _secure_config(_inner(agent)):
        setup_secure_a2a(agent)
    app = to_a2a(_inner(agent), **kwargs)
    return secure_a2a_app(app, agent)


def _install_pensieve_tools(owner: Any, agent: Any) -> None:
    if _looks_like_remote_a2a_agent(agent):
        return
    tools = append_unique_tools(
        getattr(agent, "tools", None) or [],
        make_adk_pensieve_tools(owner),
    )
    try:
        setattr(agent, "tools", tools)
    except Exception:
        if hasattr(agent, "model_copy"):
            updated = agent.model_copy(update={"tools": tools})
            agent.__dict__.update(updated.__dict__)


class WeilA2AMiddleware:
    """ASGI middleware that verifies Weil-signed A2A POST requests."""

    def __init__(
        self,
        app: Any,
        *,
        agent: Any,
        service: Optional[str] = None,
        audit: bool = True,
        verify_signature: bool = True,
    ) -> None:
        self.app = app
        self.agent = agent
        self.service = service
        self.audit = audit
        self.verify_signature = verify_signature

    async def __call__(self, scope: dict, receive: Any, send: Any) -> None:
        if scope.get("type") != "http" or scope.get("method") != "POST":
            await self.app(scope, receive, send)
            return

        body = await _read_body(receive)
        headers = _headers_from_scope(scope)
        wallet_addr = headers.get("x-wallet-address", "")

        signed_payload = _signed_payload_from_http_body(body)
        # Request shape and correlation ids only -- never message parts, which
        # carry user content that has no business on an immutable audit log.
        request = _request_context(scope, body, signed_payload)
        serving = _serving_agent_context(self.agent)

        # A request with no caller identity is rejected even when signature
        # verification is off, so no run ever starts without a named caller.
        if not wallet_addr or (
            self.verify_signature and not verify_weil_request(headers, signed_payload)
        ):
            if self.audit:
                await _audit(
                    self.agent,
                    "a2a.verify.failed",
                    outcome="rejected",
                    status=401,
                    stage="verify",
                    wallet=wallet_addr or None,
                    service=self.service,
                    **_verify_failure_context(headers, signed_payload),
                    **request,
                    **serving,
                )
            await _send_plain(
                send,
                401,
                "Wallet address verification failed for wallet "
                f"{wallet_addr or '<missing X-Wallet-Address>'!r}",
            )
            return

        # Identity lookups key on the canonical address (pod counter + key
        # hash), which the client signs. Older clients omit it; fall back to
        # the key-derived form.
        canonical_addr = full_address_from_headers(headers)
        caller = {
            "wallet": canonical_addr or wallet_addr or None,
            # Kept separate so a denial can be traced to the exact header the
            # caller sent when it differs from the signed canonical address.
            "wallet_header": wallet_addr if canonical_addr else None,
            "wallet_source": ("signed_canonical" if canonical_addr else "header")
            if (canonical_addr or wallet_addr)
            else None,
        }
        wallet_addr = canonical_addr or wallet_addr

        granted = None
        if self.service:
            check = {
                "service": self.service,
                "permission_check": "secured.key_has_purpose",
                "required_purposes": [_EXECUTION_PURPOSE, _MANAGEMENT_PURPOSE],
                "sentinel_host": getattr(self.agent, "_sentinel_host", None),
            }
            started = time.monotonic()
            try:
                granted = await _granting_purpose(
                    wallet_addr,
                    self.service,
                    purposes=(_EXECUTION_PURPOSE, _MANAGEMENT_PURPOSE),
                    sentinel_host=getattr(self.agent, "_sentinel_host"),
                    verify=getattr(self.agent, "_verify"),
                )
            except Exception as exc:
                if self.audit:
                    await _audit(
                        self.agent,
                        "a2a.permission.error",
                        outcome="error",
                        status=403,
                        stage="permission",
                        reason="secured permission check could not be completed",
                        error=str(exc),
                        error_type=type(exc).__name__,
                        check_ms=_elapsed_ms(started),
                        **caller,
                        **check,
                        **request,
                        **serving,
                    )
                await _send_plain(
                    send,
                    403,
                    f"Wallet {wallet_addr or '<unknown>'!r} failed the secured "
                    f"permission check for service {self.service!r}: {exc}",
                )
                return
            if granted is None:
                exc = WalletNotPermittedError(wallet_addr, self.service)
                if self.audit:
                    await _audit(
                        self.agent,
                        "a2a.permission.denied",
                        outcome="denied",
                        status=403,
                        stage="permission",
                        reason=(
                            f"wallet holds neither the {_EXECUTION_PURPOSE} nor the "
                            f"{_MANAGEMENT_PURPOSE} purpose on service {self.service}"
                        ),
                        check_ms=_elapsed_ms(started),
                        **caller,
                        **check,
                        **request,
                        **serving,
                    )
                await _send_plain(send, 403, str(exc))
                return

        if self.audit:
            await _audit(
                self.agent,
                "a2a.request.accepted",
                outcome="accepted",
                status=200,
                service=self.service,
                # The purpose that granted access (None when no service gate).
                purpose=granted,
                verified=self.verify_signature,
                permission_checked=bool(self.service),
                **caller,
                **request,
                **serving,
            )
        tokens = set_current_wallet_addr(
            wallet_addr,
            mode="adk",
            sentinel_host=getattr(self.agent, "_sentinel_host"),
            verify=getattr(self.agent, "_verify"),
        )
        try:
            await self.app(scope, _replay_body(body), send)
        finally:
            reset_current_wallet_addr(tokens)


def _patch_remote_agent(*, owner: Any, remote_agent: Any, audit: bool) -> None:
    if not _looks_like_remote_a2a_agent(remote_agent):
        return

    try:
        from google.adk.a2a.agent.config import RequestInterceptor
    except ImportError:
        return

    target = _remote_target_context(remote_agent)

    async def before_request(ctx: Any, a2a_request: Any, parameters: Any) -> tuple[Any, Any]:
        request_payload = _model_to_data(a2a_request)
        headers = await owner.aget_auth_headers(body=request_payload)
        _add_request_headers(parameters, headers)
        if audit:
            await _audit(
                owner,
                "a2a.request.signed",
                direction="outbound",
                wallet=_own_wallet(owner),
                signed_body_sha256=_short_hash(headers.get("X-Body-SHA256")),
                **target,
                **_message_context(request_payload),
            )
        return a2a_request, parameters

    async def after_request(ctx: Any, a2a_response: Any, event: Any) -> Any:
        # ADK calls this for every streamed event; record only the final one.
        task = a2a_response[0] if isinstance(a2a_response, tuple) else a2a_response
        payload = _model_to_data(task)
        status = payload.get("status") if isinstance(payload, dict) else None
        state = _task_state(status.get("state") if isinstance(status, dict) else None)
        if audit and (state in _FINAL_TASK_STATES or (state is None and not isinstance(a2a_response, tuple))):
            await _audit(
                owner,
                "a2a.response.received",
                direction="inbound",
                wallet=_own_wallet(owner),
                task_state=state,
                task_id=payload.get("id") if isinstance(payload, dict) else None,
                response_sha256=_short_hash(_safe_body_hash(payload)),
                **target,
            )
        return event

    interceptor = RequestInterceptor(before_request=before_request, after_request=after_request)
    config = getattr(remote_agent, "_config", None)
    if config is None:
        return

    request_interceptors = [*(getattr(config, "request_interceptors", None) or [])]
    request_interceptors.append(interceptor)
    if hasattr(config, "model_copy"):
        remote_agent._config = config.model_copy(
            update={"request_interceptors": request_interceptors}
        )
    else:
        config.request_interceptors = request_interceptors


def _compose_agent_callbacks(
    weil_agent: Any,
    inner: Any,
    *,
    service: Optional[str],
    audit: bool,
    parent: Any = None,
) -> None:
    """Audit agent runs under *weil_agent*'s wallet and scope its secured tools.

    The caller (set by the inbound middleware) is authorized at the A2A
    boundary; inside the agent, @secured tools authorize as the agent itself.
    """
    _compose_tool_callbacks(weil_agent, inner, audit=audit, parent=parent)
    if not audit:
        return

    before = getattr(inner, "before_agent_callback", None)
    after = getattr(inner, "after_agent_callback", None)
    trace_handles: dict[Any, Any] = {}

    async def before_wrapper(*args: Any, **kwargs: Any) -> Any:
        ctx = _first_context(args, kwargs)
        run_id = _callback_run_key(ctx)
        trace = begin_trace(weil_agent, _trace_root(ctx))
        trace_handles[run_id] = trace
        _set_trace_handle(ctx, trace)
        if audit:
            await _audit(
                weil_agent,
                "adk.agent.before",
                agent=getattr(inner, "name", ""),
                agent_type=type(inner).__name__,
                service=service,
                wallet=_own_wallet(weil_agent),
                # Set by the inbound middleware when this run serves an A2A call,
                # so a run can be traced back to the caller that triggered it.
                caller_wallet=_caller_wallet(weil_agent, parent),
            )
        return await _maybe_call(before, *args, **kwargs)

    async def after_wrapper(*args: Any, **kwargs: Any) -> Any:
        ctx = _first_context(args, kwargs)
        run_id = _callback_run_key(ctx)
        try:
            result = await _maybe_call(after, *args, **kwargs)
            if audit:
                await _audit(
                    weil_agent,
                    "adk.agent.after",
                    agent=getattr(inner, "name", ""),
                    agent_type=type(inner).__name__,
                    service=service,
                    wallet=_own_wallet(weil_agent),
                    caller_wallet=_caller_wallet(weil_agent, parent),
                )
            return result
        finally:
            trace = trace_handles.pop(run_id, None) or _get_trace_handle(ctx)
            if trace is not None:
                try:
                    await finish_trace(trace)
                except Exception:
                    pass

    _set_callback(inner, "before_agent_callback", before_wrapper)
    _set_callback(inner, "after_agent_callback", after_wrapper)


def _compose_tool_callbacks(
    weil_agent: Any, inner: Any, *, audit: bool, parent: Any = None
) -> None:
    """Run @secured tools as *weil_agent* and audit them with their arguments.

    Only tools guarded by ``@secured_user``/``@secured_admin`` are touched;
    other tools pass straight through to any callbacks already installed.

    A secured tool runs with the acting agent's wallet in the ContextVar. The
    token returned when it is set is kept until ADK's matching after-tool
    callback, which restores the previous context. The callbacks are paired by
    ADK's function-call ID.
    """
    if not hasattr(inner, "before_tool_callback"):
        return

    before = getattr(inner, "before_tool_callback", None)
    after = getattr(inner, "after_tool_callback", None)
    # Pair each before-tool context token with its after-tool callback.
    tokens: dict[Any, Any] = {}
    tool_trace_contexts: dict[Any, tuple[str, Optional[str], Any]] = {}

    async def before_tool_wrapper(*args: Any, **kwargs: Any) -> Any:
        tool, tool_args, tool_context = _tool_callback_args(args, kwargs)
        event_id = str(uuid.uuid4())
        parent_id = current_parent_id()
        parent_token = set_trace_parent(event_id)
        _store_tool_trace_context(
            tool_trace_contexts,
            tool,
            tool_context,
            event_id,
            parent_id,
            parent_token,
        )
        secured = _secured_tool_info(tool)
        try:
            if secured is not None:
                if audit:
                    await _audit(
                        weil_agent,
                        "adk.tool.before",
                        **_tool_context(weil_agent, inner, tool, tool_context, secured, parent),
                        args=tool_args,
                    )
                tokens[tool_context.function_call_id] = set_current_wallet_addr(
                    _own_wallet(weil_agent),
                    mode="adk",
                    sentinel_host=weil_agent._sentinel_host,
                    verify=weil_agent._verify,
                )
            return await _maybe_call(before, *args, **kwargs)
        except Exception:
            _restore_tool_trace_context(tool_trace_contexts, tool, tool_context)
            raise

    async def after_tool_wrapper(*args: Any, **kwargs: Any) -> Any:
        tool, tool_args, tool_context = _tool_callback_args(args, kwargs)
        trace_ctx = _pop_tool_trace_context(tool_trace_contexts, tool, tool_context)
        event_id, parent_id, parent_token = trace_ctx or (
            str(uuid.uuid4()),
            current_parent_id(),
            None,
        )
        restored = False
        try:
            result = await _maybe_call(after, *args, **kwargs)
            if parent_token is not None:
                _reset_trace_parent_token(parent_token)
                restored = True
            response = kwargs.get("tool_response", args[3] if len(args) > 3 else None)
            final_response = result if result is not None else response
            await arecord_tool_call(
                getattr(tool, "name", None),
                _tool_args_with_metadata(tool_args, _tool_call_id(tool_context)),
                final_response,
                event_id=event_id,
                parent_id=parent_id,
            )
            secured = _secured_tool_info(tool)
            if audit and secured is not None:
                await _audit(
                    weil_agent,
                    "adk.tool.after",
                    **_tool_context(weil_agent, inner, tool, tool_context, secured, parent),
                    # A user after-callback may replace the tool result; record
                    # what the model actually receives.
                    response=final_response,
                )
            return result
        finally:
            if parent_token is not None and not restored:
                _reset_trace_parent_token(parent_token)
            if caller_tokens := tokens.pop(tool_context.function_call_id, None):
                reset_current_wallet_addr(caller_tokens)

    _set_callback(inner, "before_tool_callback", before_tool_wrapper)
    _set_callback(inner, "after_tool_callback", after_tool_wrapper)


def _tool_callback_args(args: tuple, kwargs: dict) -> tuple[Any, Any, Any]:
    """(tool, args, tool_context) from an ADK tool callback, positional or keyword."""
    positional = list(args) + [None] * 3
    return (
        kwargs.get("tool", positional[0]),
        kwargs.get("args", positional[1]),
        kwargs.get("tool_context", positional[2]),
    )


def _tool_call_id(tool_context: Any) -> Optional[str]:
    for attr in ("function_call_id", "tool_call_id", "call_id", "id"):
        value = getattr(tool_context, attr, None)
        if value:
            return str(value)
    return None


def _tool_args_with_metadata(args: Any, framework_call_id: Optional[str]) -> Any:
    if not framework_call_id:
        return args
    if isinstance(args, dict):
        return {
            **args,
            "_weil": {
                "framework": "adk",
                "tool_call_id": framework_call_id,
            },
        }
    return {
        "value": args,
        "_weil": {
            "framework": "adk",
            "tool_call_id": framework_call_id,
        },
    }


def _tool_trace_key(tool: Any, tool_context: Any) -> Any:
    call_id = _tool_call_id(tool_context)
    if call_id:
        return ("call", call_id)
    return ("context", id(tool_context), getattr(tool, "name", None))


def _store_tool_trace_context(
    contexts: dict[Any, tuple[str, Optional[str], Any]],
    tool: Any,
    tool_context: Any,
    event_id: str,
    parent_id: Optional[str],
    parent_token: Any,
) -> None:
    ctx = (event_id, parent_id, parent_token)
    contexts[_tool_trace_key(tool, tool_context)] = ctx
    if tool_context is None:
        return
    try:
        setattr(tool_context, "_weil_trace_context", ctx)
    except Exception:
        return


def _pop_tool_trace_context(
    contexts: dict[Any, tuple[str, Optional[str], Any]],
    tool: Any,
    tool_context: Any,
) -> Optional[tuple[str, Optional[str], Any]]:
    key = _tool_trace_key(tool, tool_context)
    ctx = contexts.pop(key, None)
    if ctx is None and tool_context is not None:
        ctx = getattr(tool_context, "_weil_trace_context", None)
    if tool_context is not None:
        try:
            delattr(tool_context, "_weil_trace_context")
        except Exception:
            pass
    return ctx


def _restore_tool_trace_context(
    contexts: dict[Any, tuple[str, Optional[str], Any]],
    tool: Any,
    tool_context: Any,
) -> None:
    ctx = _pop_tool_trace_context(contexts, tool, tool_context)
    if ctx is not None:
        _reset_trace_parent_token(ctx[2])


def _reset_trace_parent_token(token: Any) -> None:
    try:
        reset_trace_parent(token)
    except ValueError:
        # ADK may call before/after hooks in different contextvars contexts.
        # The token's original context is already gone, so there is nothing
        # useful left to restore here.
        pass


def _trace_root(ctx: Any) -> Any:
    """Choose the root value used to initialize an ADK run trace.

    Prefer user prompt text when it can be extracted from the callback context;
    otherwise fall back to ADK invocation metadata, then to the context type.
    """
    prompt = extract_prompt(ctx)
    if prompt:
        return prompt
    invocation = _invocation_context(ctx)
    return invocation or {"type": type(ctx).__name__ if ctx is not None else "unknown"}


def _set_trace_handle(ctx: Any, trace: Any) -> None:
    """Attach the active trace handle to an ADK callback context.

    ADK passes context objects between before/after callbacks, so storing the
    handle there lets the after callback finish the same trace even when
    contextvars are not enough to identify it.
    """
    if ctx is None or not getattr(trace, "active", False):
        return
    try:
        setattr(ctx, "_weil_trace_handle", trace)
    except Exception:
        return


def _get_trace_handle(ctx: Any) -> Any:
    """Return a trace handle previously attached to an ADK context."""
    if ctx is None:
        return None
    return getattr(ctx, "_weil_trace_handle", None)


def _tool_context(
    weil: Any,
    inner: Any,
    tool: Any,
    tool_context: Any,
    secured: dict[str, str],
    parent: Any = None,
) -> dict[str, Any]:
    return {
        "agent": getattr(inner, "name", None) or None,
        "tool": getattr(tool, "name", None),
        "tool_call_id": _tool_call_id(tool_context),
        **secured,
        "wallet": _own_wallet(weil),
        "caller_wallet": _caller_wallet(weil, parent),
    }


def _add_request_headers(parameters: Any, headers: dict[str, str]) -> None:
    if getattr(parameters, "client_call_context", None) is None:
        try:
            # a2a-sdk >= 1.0
            from a2a.client import ClientCallContext
        except ImportError:
            try:
                # a2a-sdk < 1.0
                from a2a.client.middleware import ClientCallContext
            except ImportError as exc:
                # Never fall through silently: without a call context the
                # signed auth headers are dropped and the request goes out
                # unauthenticated.
                raise RuntimeError(
                    "weil_ai: cannot import ClientCallContext from a2a-sdk; "
                    "refusing to send an unsigned A2A request"
                ) from exc
        parameters.client_call_context = ClientCallContext()

    client_call_context = parameters.client_call_context
    if hasattr(client_call_context, "service_parameters"):
        service_parameters = getattr(client_call_context, "service_parameters", None)
        client_call_context.service_parameters = {
            **(service_parameters or {}),
            **headers,
        }
        return

    state = dict(getattr(client_call_context, "state", None) or {})
    http_kwargs = dict(state.get("http_kwargs") or {})
    http_kwargs["headers"] = {**(http_kwargs.get("headers") or {}), **headers}
    state["http_kwargs"] = http_kwargs
    client_call_context.state = state


async def _audit(weil: Any, event: str, **fields: Any) -> None:
    """Write one audit record. Fields that are None are dropped, so callers can
    pass optional context unconditionally. Inside a traced run the record also
    carries trace_id, linking it to the trace published for that run."""
    try:
        payload = {
            "event": event,
            "ts": _now_iso(),
            **{k: v for k, v in {"trace_id": current_trace_id(), **fields}.items() if v is not None},
        }
        result = weil._audit_async(json.dumps(payload, default=repr))
        if inspect.isawaitable(result):
            await result
    except Exception:
        return


# ----------------------------------------------------------------------
# Audit context builders
#
# Everything below extracts *metadata* -- addresses, service names, ids,
# sizes, hashes, outcomes. Message parts, header signatures and the signed
# X-Message payload are deliberately left out: audit records are immutable
# and public to anyone who can read the auditor applet. The one exception is
# @secured tool calls (_compose_tool_callbacks), whose arguments and response
# are recorded by design.
# ----------------------------------------------------------------------


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace(
        "+00:00", "Z"
    )


def _elapsed_ms(started: float) -> int:
    return int((time.monotonic() - started) * 1000)


def _short_hash(value: Any) -> Optional[str]:
    """First 16 hex chars of a digest -- enough to correlate, a third of the size."""
    if not isinstance(value, str) or not value:
        return None
    return value[:16]


def _own_wallet(weil: Any) -> Optional[str]:
    try:
        return weil._wallet_auth_address()
    except Exception:
        return None


def _caller_wallet(weil: Any, parent: Any = None) -> Optional[str]:
    """Wallet of whoever called *weil*: its enclosing agent when embedded, else
    the A2A caller currently being served, if any."""
    try:
        caller = _own_wallet(parent) if parent is not None else current_wallet_addr()
    except Exception:
        return None
    if not caller or caller == _own_wallet(weil):
        return None
    return caller


def _serving_agent_context(weil: Any) -> dict[str, Any]:
    inner = _inner(weil)
    return {
        "agent": getattr(inner, "name", None) or None,
        "agent_wallet": _own_wallet(weil),
    }


def _request_context(scope: dict, body: bytes, signed_payload: Any) -> dict[str, Any]:
    """HTTP + JSON-RPC envelope facts for an inbound A2A request."""
    client = scope.get("client") or ()
    headers = _headers_from_scope(scope)
    context: dict[str, Any] = {
        "path": scope.get("path") or None,
        "peer": client[0] if client else None,
        "user_agent": headers.get("user-agent") or None,
        "body_bytes": len(body) or None,
        "body_sha256": _short_hash(_safe_body_hash(signed_payload)),
    }
    try:
        envelope = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        envelope = None
    if isinstance(envelope, dict):
        context["rpc_method"] = envelope.get("method") or None
        context["rpc_id"] = envelope.get("id") or None
    context.update(_message_context(signed_payload))
    return context


def _message_context(payload: Any) -> dict[str, Any]:
    """Correlation ids from an A2A message (camelCase or snake_case)."""
    if not isinstance(payload, dict):
        return {}
    return {
        "message_id": payload.get("messageId") or payload.get("message_id"),
        "task_id": payload.get("taskId") or payload.get("task_id"),
        "context_id": payload.get("contextId") or payload.get("context_id"),
    }


# Terminal task states: no further events are expected. input_required and
# auth_required only pause the task (it resumes on a follow-up), so they are not here.
_FINAL_TASK_STATES = {
    "completed",
    "failed",
    "canceled",
    "cancelled",
    "rejected",
}


def _task_state(state: Any) -> Optional[str]:
    """Normalise an A2A task state ("completed", "TASK_STATE_COMPLETED", ...)."""
    if not state:
        return None
    return str(state).lower().removeprefix("task_state_")


def _remote_target_context(remote_agent: Any) -> dict[str, Any]:
    config = getattr(remote_agent, "_config", None)
    agent_card = getattr(config, "agent_card", None)
    url = getattr(config, "url", None) or getattr(agent_card, "url", None)
    if url is None and isinstance(agent_card, str):
        url = agent_card
    return {
        "target": getattr(remote_agent, "name", None) or None,
        "target_url": url,
    }


def _first_context(args: tuple, kwargs: dict) -> Any:
    """The ADK CallbackContext, wherever the framework chose to pass it."""
    if args:
        return args[0]
    for key in ("callback_context", "context", "ctx"):
        if key in kwargs:
            return kwargs[key]
    return None


def _callback_run_key(ctx: Any) -> Any:
    return _invocation_context(ctx).get("invocation_id") or (
        id(ctx) if ctx is not None else None
    )


def _invocation_context(ctx: Any) -> dict[str, Any]:
    """ADK run correlation ids, best effort across ADK versions."""
    if ctx is None:
        return {}
    invocation = getattr(ctx, "_invocation_context", None)
    session = getattr(invocation, "session", None) or getattr(ctx, "session", None)
    return {
        "invocation_id": getattr(ctx, "invocation_id", None)
        or getattr(invocation, "invocation_id", None),
        "session_id": getattr(session, "id", None),
        "app_name": getattr(session, "app_name", None)
        or getattr(invocation, "app_name", None),
        "user_id": getattr(session, "user_id", None)
        or getattr(invocation, "user_id", None),
    }


def _verify_failure_context(headers: dict[str, str], signed_payload: Any) -> dict[str, Any]:
    """Why verification failed, so a 401 is actionable without a repro.

    Distinguishes the three failures that look identical from the outside:
    headers never sent, a stale/replayed timestamp, and a body that does not
    match the one that was signed. Anything else is a signature mismatch.
    """
    missing = [name for name in _REQUIRED_AUTH_HEADERS if not headers.get(name)]
    if missing:
        return {"reason": "missing_auth_headers", "missing_headers": missing}

    try:
        skew = int(time.time()) - int(headers["x-timestamp"])
    except (TypeError, ValueError):
        return {"reason": "malformed_timestamp", "timestamp": headers.get("x-timestamp")}
    if abs(skew) > MAX_TIMESTAMP_AGE_SECONDS:
        return {
            "reason": "stale_timestamp",
            "timestamp_skew_s": skew,
            "max_age_s": MAX_TIMESTAMP_AGE_SECONDS,
        }

    try:
        signed = json.loads(headers["x-message"])
    except (TypeError, ValueError):
        return {"reason": "malformed_signed_message"}
    if not isinstance(signed, dict):
        return {"reason": "malformed_signed_message"}

    signed_hash = signed.get("body_sha256")
    actual_hash = _safe_body_hash(signed_payload)
    if signed_hash and actual_hash and signed_hash != actual_hash:
        return {
            "reason": "body_hash_mismatch",
            "signed_body_sha256": _short_hash(signed_hash),
            "actual_body_sha256": _short_hash(actual_hash),
        }
    if not signed_hash:
        return {"reason": "unbound_body", "detail": "signed message omits body_sha256"}

    return {"reason": "signature_mismatch"}


def _safe_body_hash(payload: Any) -> Optional[str]:
    try:
        return _body_sha256(payload)
    except Exception:
        return None


async def _maybe_call(fn: Any, *args: Any, **kwargs: Any) -> Any:
    if fn is None:
        return None
    if isinstance(fn, list):
        result = None
        for callback in fn:
            result = await _maybe_call(callback, *args, **kwargs)
            if result is not None:
                return result
        return result
    result = fn(*args, **kwargs)
    if inspect.isawaitable(result):
        return await result
    return result


def _set_callback(obj: Any, name: str, value: Any) -> None:
    try:
        setattr(obj, name, value)
    except Exception:
        if hasattr(obj, "model_copy"):
            updated = obj.model_copy(update={name: value})
            obj.__dict__.update(updated.__dict__)


def _has_own_identity(agent: Any) -> bool:
    """Whether *agent* was wrapped with weil_ai.agent, so it carries its own wallet."""
    return hasattr(agent, "get_auth_headers") and hasattr(agent, "_audit_async")


def _as_weil_agent(agent: Any) -> Any:
    if not _has_own_identity(agent):
        raise TypeError("setup_secure_a2a() requires an object wrapped with weil_ai.agent")
    return agent


def _inner(agent: Any) -> Any:
    return getattr(agent, "inner_agent", agent)


def _sub_agents(agent: Any) -> list[Any]:
    """Agents *agent* delegates to: its sub_agents plus any wrapped as AgentTool."""
    agents = list(getattr(agent, "sub_agents", None) or [])
    for tool in getattr(agent, "tools", None) or []:
        if type(tool).__name__ == "AgentTool" and getattr(tool, "agent", None) is not None:
            agents.append(tool.agent)
    return agents


def _secure_config(agent: Any) -> dict[str, Any]:
    return getattr(agent, _SECURE_A2A_ATTR, {}) or {}


def _looks_like_remote_a2a_agent(agent: Any) -> bool:
    cls_name = agent.__class__.__name__.lower()
    return "remote" in cls_name and "a2a" in cls_name


def _model_to_data(value: Any) -> Any:
    if hasattr(value, "model_dump"):
        return value.model_dump(mode="json", by_alias=True, exclude_none=True)
    if hasattr(value, "dict"):
        return value.dict(by_alias=True, exclude_none=True)
    if hasattr(value, "DESCRIPTOR"):
        # protobuf message (a2a-sdk >= 1.0 uses protobuf, not pydantic).
        # Must mirror the wire encoding exactly -- a2a's JSONRPC transport
        # serialises with MessageToDict() defaults (camelCase) -- or the
        # signed body hash will not match what the server receives.
        from google.protobuf.json_format import MessageToDict

        return MessageToDict(value)
    return value


def _signed_payload_from_http_body(body: bytes) -> Any:
    try:
        parsed = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return body
    if not isinstance(parsed, dict):
        return parsed

    params = parsed.get("params")
    if isinstance(params, dict):
        for key in ("message", "request"):
            value = params.get(key)
            if isinstance(value, dict):
                return value
        return params

    for key in ("message", "request"):
        value = parsed.get(key)
        if isinstance(value, dict):
            return value
    return parsed


async def _read_body(receive: Any) -> bytes:
    chunks = []
    more_body = True
    while more_body:
        message = await receive()
        chunks.append(message.get("body", b""))
        more_body = message.get("more_body", False)
    return b"".join(chunks)


def _replay_body(body: bytes) -> Any:
    """Re-feed an already-drained request body to the downstream ASGI app.

    The middleware consumes the original receive() stream to hash the body, so
    the app needs the bytes handed back. After that single replay there are no
    further client messages: returning another http.request immediately would
    busy-loop any app that polls receive() to watch for a disconnect -- which
    is exactly what the A2A server does while streaming an SSE response. So
    later calls block instead, and are cancelled when the request is torn down.
    """
    sent = False

    async def receive() -> dict:
        nonlocal sent
        if sent:
            await asyncio.Event().wait()  # never set; cancelled on teardown
        sent = True
        return {"type": "http.request", "body": body, "more_body": False}

    return receive


def _headers_from_scope(scope: dict) -> dict[str, str]:
    return {
        key.decode("latin1").lower(): value.decode("latin1")
        for key, value in scope.get("headers", [])
    }


async def _send_plain(send: Any, status: int, body: Any) -> None:
    if isinstance(body, str):
        body = body.encode("utf-8")
    await send(
        {
            "type": "http.response.start",
            "status": status,
            "headers": [(b"content-type", b"text/plain; charset=utf-8")],
        }
    )
    await send({"type": "http.response.body", "body": body})
