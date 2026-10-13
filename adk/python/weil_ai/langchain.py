"""LangChain integration for Weil identity, secured tools, and audit hooks."""

from __future__ import annotations

import contextlib
import contextvars
import inspect
import json
from typing import Any, Iterator, Optional

from .mcp import _secured_tool_info, reset_current_wallet_addr, set_current_wallet_addr
from .memory import append_unique_tools, make_langchain_pensieve_tools


_FACTORY_IDENTITY: contextvars.ContextVar[Any] = contextvars.ContextVar(
    "weil_langchain_factory_identity",
    default=None,
)


@contextlib.contextmanager
def _factory_identity(agent: Any) -> Iterator[None]:
    token = _FACTORY_IDENTITY.set(agent)
    try:
        yield
    finally:
        _FACTORY_IDENTITY.reset(token)


def create_agent(
    *args: Any,
    middleware: tuple[Any, ...] | list[Any] = (),
    pensieve: bool = True,
    **kwargs: Any,
) -> Any:
    """Create a LangChain agent with Weil middleware injected.

    Use this inside a ``@weil_ai.agent(...)`` factory. The helper preserves normal
    LangChain ``create_agent`` arguments and prepends Weil's middleware.
    """

    try:
        from langchain.agents import create_agent as langchain_create_agent
    except ImportError as exc:
        raise ImportError(
            "langchain is required for weil_ai.langchain.create_agent(); install langchain"
        ) from exc

    owner = _current_identity()
    weil_middleware = secure_middleware(owner)
    if pensieve:
        args, kwargs = _add_pensieve_tools(owner, args, kwargs)
    return langchain_create_agent(
        *args,
        middleware=[weil_middleware, *list(middleware or [])],
        **kwargs,
    )


def secure_middleware(
    owner: Any = None,
    *,
    audit: bool = True,
    verify: bool = True,
) -> Any:
    """Return LangChain AgentMiddleware for Weil audit and secured tool context."""

    try:
        from langchain.agents.middleware import AgentMiddleware
    except ImportError as exc:
        raise ImportError(
            "langchain is required for Weil LangChain middleware; install langchain"
        ) from exc

    resolved_owner = owner or _current_identity()

    class WeilLangChainMiddleware(AgentMiddleware):
        @property
        def name(self) -> str:
            return "WeilLangChainMiddleware"

        def before_model(self, state: Any, runtime: Any) -> None:
            if audit:
                _run_sync_audit(
                    resolved_owner,
                    "langchain.before_model",
                    state=_state_summary(state),
                )
            return None

        async def abefore_model(self, state: Any, runtime: Any) -> None:
            if audit:
                await _audit(
                    resolved_owner,
                    "langchain.before_model",
                    state=_state_summary(state),
                )
            return None

        def after_model(self, state: Any, runtime: Any) -> None:
            if audit:
                _run_sync_audit(
                    resolved_owner,
                    "langchain.after_model",
                    state=_state_summary(state),
                )
            return None

        async def aafter_model(self, state: Any, runtime: Any) -> None:
            if audit:
                await _audit(
                    resolved_owner,
                    "langchain.after_model",
                    state=_state_summary(state),
                )
            return None

        def wrap_tool_call(self, request: Any, handler: Any) -> Any:
            tokens = _set_langchain_wallet_context(resolved_owner, verify=verify)
            try:
                if audit:
                    _run_sync_audit(
                        resolved_owner,
                        "langchain.tool.before",
                        tool=_tool_name(request),
                        **_secured_tool_args(request),
                    )
                result = handler(request)
                if audit:
                    _run_sync_audit(
                        resolved_owner,
                        "langchain.tool.after",
                        tool=_tool_name(request),
                        **_secured_tool_response(request, result),
                    )
                return result
            finally:
                reset_current_wallet_addr(tokens)

        async def awrap_tool_call(self, request: Any, handler: Any) -> Any:
            tokens = _set_langchain_wallet_context(resolved_owner, verify=verify)
            try:
                if audit:
                    await _audit(
                        resolved_owner,
                        "langchain.tool.before",
                        tool=_tool_name(request),
                        **_secured_tool_args(request),
                    )
                result = await handler(request)
                if audit:
                    await _audit(
                        resolved_owner,
                        "langchain.tool.after",
                        tool=_tool_name(request),
                        **_secured_tool_response(request, result),
                    )
                return result
            finally:
                reset_current_wallet_addr(tokens)

    return WeilLangChainMiddleware()


def setup_secure_langchain(
    agent: Any,
    *,
    audit: bool = True,
    verify: bool = True,
    pensieve: bool = True,
) -> Any:
    """Fallback setup for already-created LangChain agents.

    LangChain's model hooks are compiled into the graph at ``create_agent(...)``
    time. For full ``before_model``/``after_model`` behavior, use
    ``weil_ai.langchain.create_agent`` inside a ``@weil_ai.agent`` factory.
    This fallback wraps top-level invocation so secured tools still see the
    Weil wallet context when the graph invokes synchronously/asynchronously.
    """

    owner = _as_weil_agent(agent)
    inner = getattr(owner, "inner_agent", agent)
    if pensieve:
        _install_pensieve_tools(owner, inner)

    if getattr(inner, "__weil_langchain_wrapped__", False):
        return agent

    for name in ("invoke", "stream"):
        if hasattr(inner, name):
            setattr(inner, name, _wrap_sync_invocation(owner, getattr(inner, name), audit, verify))

    for name in ("ainvoke", "astream"):
        if hasattr(inner, name):
            setattr(inner, name, _wrap_async_invocation(owner, getattr(inner, name), audit, verify))

    setattr(inner, "__weil_langchain_wrapped__", True)
    return agent


def _add_pensieve_tools(
    owner: Any,
    args: tuple[Any, ...],
    kwargs: dict[str, Any],
) -> tuple[tuple[Any, ...], dict[str, Any]]:
    additions = make_langchain_pensieve_tools(owner)
    if len(args) >= 2:
        updated = list(args)
        updated[1] = append_unique_tools(updated[1], additions)
        return tuple(updated), kwargs

    updated_kwargs = dict(kwargs)
    updated_kwargs["tools"] = append_unique_tools(
        updated_kwargs.get("tools"),
        additions,
    )
    return args, updated_kwargs


def _install_pensieve_tools(owner: Any, agent: Any) -> None:
    if not hasattr(agent, "tools"):
        return
    try:
        tools = append_unique_tools(
            getattr(agent, "tools", None) or [],
            make_langchain_pensieve_tools(owner),
        )
        setattr(agent, "tools", tools)
    except Exception:
        return


def _wrap_sync_invocation(owner: Any, fn: Any, audit: bool, verify: bool) -> Any:
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        tokens = _set_langchain_wallet_context(owner, verify=verify)
        try:
            if audit:
                _run_sync_audit(owner, "langchain.invoke.before")
            result = fn(*args, **kwargs)
            if audit:
                _run_sync_audit(owner, "langchain.invoke.after")
            return result
        finally:
            reset_current_wallet_addr(tokens)

    return wrapper


def _wrap_async_invocation(owner: Any, fn: Any, audit: bool, verify: bool) -> Any:
    async def wrapper(*args: Any, **kwargs: Any) -> Any:
        tokens = _set_langchain_wallet_context(owner, verify=verify)
        try:
            if audit:
                await _audit(owner, "langchain.invoke.before")
            result = fn(*args, **kwargs)
            if inspect.isawaitable(result):
                result = await result
            if audit:
                await _audit(owner, "langchain.invoke.after")
            return result
        finally:
            reset_current_wallet_addr(tokens)

    return wrapper


def _current_identity() -> Any:
    owner = _FACTORY_IDENTITY.get()
    if owner is None:
        raise RuntimeError(
            "No active Weil identity. Use weil_ai.langchain.create_agent(...) "
            "inside a @weil_ai.agent(...) factory, or pass owner= to secure_middleware()."
        )
    return owner


def _as_weil_agent(agent: Any) -> Any:
    if hasattr(agent, "get_auth_headers") and hasattr(agent, "_audit_async"):
        return agent
    raise TypeError("setup_secure_langchain() requires an object wrapped with weil_ai.agent")


def _set_langchain_wallet_context(owner: Any, *, verify: bool) -> tuple[Any, Any, Any, Any]:
    return set_current_wallet_addr(
        owner._wallet_auth_address(),
        mode="langchain",
        sentinel_host=getattr(owner, "_sentinel_host"),
        verify=getattr(owner, "_verify") if verify else False,
    )


async def _audit(owner: Any, event: str, **fields: Any) -> None:
    try:
        payload = {"event": event, **{k: v for k, v in fields.items() if v is not None}}
        result = owner._audit_async(json.dumps(payload, default=repr))
        if inspect.isawaitable(result):
            await result
    except Exception:
        return


def _run_sync_audit(owner: Any, event: str, **fields: Any) -> None:
    try:
        from .mcp import _run_sync

        _run_sync(_audit(owner, event, **fields))
    except Exception:
        return


def _state_summary(state: Any) -> dict[str, Any]:
    if not isinstance(state, dict):
        return {"type": type(state).__name__}
    messages = state.get("messages") or []
    return {
        "keys": sorted(str(key) for key in state.keys()),
        "message_count": len(messages) if hasattr(messages, "__len__") else None,
    }


def _tool_name(request: Any) -> Optional[str]:
    tool = getattr(request, "tool", None)
    if tool is not None:
        return getattr(tool, "name", None)
    call = getattr(request, "tool_call", None)
    if isinstance(call, dict):
        return call.get("name")
    return None


def _tool_call_id(request: Any) -> Optional[str]:
    call = getattr(request, "tool_call", None)
    if isinstance(call, dict):
        return call.get("id")
    return None


def _secured_tool_args(request: Any) -> dict[str, Any]:
    """Arguments of a @secured tool call, for the before-tool audit record."""
    secured = _secured_tool_info(getattr(request, "tool", None))
    if secured is None:
        return {}
    call = getattr(request, "tool_call", None)
    return {
        **secured,
        "tool_call_id": _tool_call_id(request),
        "args": call.get("args") if isinstance(call, dict) else None,
    }


def _secured_tool_response(request: Any, result: Any) -> dict[str, Any]:
    """Response of a @secured tool call, for the after-tool audit record."""
    secured = _secured_tool_info(getattr(request, "tool", None))
    if secured is None:
        return {}
    return {
        **secured,
        "tool_call_id": _tool_call_id(request),
        "status": getattr(result, "status", None),
        "response": _decode_json(getattr(result, "content", result)),
    }


def _decode_json(value: Any) -> Any:
    """ToolMessage content is JSON-stringified; decode it so the record nests
    the tool's structured output instead of an escaped string."""
    if not isinstance(value, str):
        return value
    try:
        return json.loads(value)
    except ValueError:
        return value
