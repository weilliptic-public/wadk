"""LangChain integration for Weil identity, secured tools, and audit hooks."""

from __future__ import annotations

import contextlib
import contextvars
import inspect
import json
import uuid
import time
from typing import Any, Iterator, Optional, Union

from .adk import _caller_wallet, _elapsed_ms, _now_iso, _own_wallet
from .mcp import _secured_tool_info, reset_current_wallet_addr, set_current_wallet_addr
from .memory import append_unique_tools, make_langchain_pensieve_tools
from .tracer import (
    arecord_tool_call,
    begin_trace,
    current_parent_id,
    finish_trace,
    finish_trace_sync,
    is_async_stream,
    is_sync_stream,
    record_tool_call,
    root_from_call,
    trace_parent,
)


_FACTORY_IDENTITY: contextvars.ContextVar[Any] = contextvars.ContextVar(
    "weil_langchain_factory_identity",
    default=None,
)

_SECURE_A2A_ATTR = "__weil_secure_a2a__"


def _secure_config(agent: Any) -> dict[str, Any]:
    return getattr(agent, _SECURE_A2A_ATTR, {}) or {}


@contextlib.contextmanager
def _factory_identity(agent: Any) -> Iterator[None]:
    token = _FACTORY_IDENTITY.set(agent)
    try:
        yield
    finally:
        _FACTORY_IDENTITY.reset(token)


def create_agent(
    *args: Any,
    middleware: Union[tuple[Any, ...], list[Any]] = (),
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
    agent = langchain_create_agent(
        *args,
        middleware=[weil_middleware, *list(middleware or [])],
        **kwargs,
    )
    _wrap_invocations(owner, agent, audit=False, verify=True)
    return agent


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

        def before_agent(self, state: Any, runtime: Any) -> None:
            if audit:
                _run_sync_audit(
                    resolved_owner,
                    "langchain.agent.before",
                    **_agent_context(resolved_owner, state),
                )
            return None

        async def abefore_agent(self, state: Any, runtime: Any) -> None:
            if audit:
                await _audit(
                    resolved_owner,
                    "langchain.agent.before",
                    **_agent_context(resolved_owner, state),
                )
            return None

        def after_agent(self, state: Any, runtime: Any) -> None:
            if audit:
                _run_sync_audit(
                    resolved_owner,
                    "langchain.agent.after",
                    **_agent_context(resolved_owner, state, final=True),
                )
            return None

        async def aafter_agent(self, state: Any, runtime: Any) -> None:
            if audit:
                await _audit(
                    resolved_owner,
                    "langchain.agent.after",
                    **_agent_context(resolved_owner, state, final=True),
                )
            return None

        def wrap_tool_call(self, request: Any, handler: Any) -> Any:
            tokens = _set_langchain_wallet_context(resolved_owner, verify=verify)
            parent_id = current_parent_id()
            event_id = str(uuid.uuid4())
            framework_call_id = _tool_call_id(request)
            started = time.monotonic()
            try:
                if audit:
                    _run_sync_audit(
                        resolved_owner,
                        "langchain.tool.before",
                        **{**_tool_context(resolved_owner, request), **_secured_tool_args(request)},
                    )
                with trace_parent(event_id):
                    try:
                        result = handler(request)
                    except Exception as exc:
                        if audit:
                            _run_sync_audit(
                                resolved_owner,
                                "langchain.tool.after",
                                **_tool_context(resolved_owner, request),
                                status="error",
                                error=type(exc).__name__,
                                error_message=str(exc),
                                elapsed_ms=_elapsed_ms(started),
                            )
                        raise
                record_tool_call(
                    _tool_name(request),
                    _tool_args_with_metadata(request, framework_call_id),
                    _tool_result(result),
                    event_id=event_id,
                    parent_id=parent_id,
                )
                
                if audit:
                    _run_sync_audit(
                        resolved_owner,
                        "langchain.tool.after",
                        **{**_tool_context(resolved_owner, request), **_secured_tool_response(request, result)},
                        elapsed_ms=_elapsed_ms(started),
                    )
                return result
            finally:
                reset_current_wallet_addr(tokens)

        async def awrap_tool_call(self, request: Any, handler: Any) -> Any:
            tokens = _set_langchain_wallet_context(resolved_owner, verify=verify)
            parent_id = current_parent_id()
            event_id = str(uuid.uuid4())
            framework_call_id = _tool_call_id(request)
            started = time.monotonic()
            try:
                if audit:
                    await _audit(
                        resolved_owner,
                        "langchain.tool.before",
                        **{**_tool_context(resolved_owner, request), **_secured_tool_args(request)},
                    )
                with trace_parent(event_id):
                    try:
                        result = await handler(request)
                    except Exception as exc:
                        if audit:
                            await _audit(
                                resolved_owner,
                                "langchain.tool.after",
                                **_tool_context(resolved_owner, request),
                                status="error",
                                error=type(exc).__name__,
                                error_message=str(exc),
                                elapsed_ms=_elapsed_ms(started),
                            )
                        raise
                await arecord_tool_call(
                    _tool_name(request),
                    _tool_args_with_metadata(request, framework_call_id),
                    _tool_result(result),
                    event_id=event_id,
                    parent_id=parent_id,
                )
                if audit:
                    await _audit(
                        resolved_owner,
                        "langchain.tool.after",
                        **{**_tool_context(resolved_owner, request), **_secured_tool_response(request, result)},
                        elapsed_ms=_elapsed_ms(started),
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

    LangChain middleware is compiled into the graph at ``create_agent(...)``
    time, so per-tool hooks need ``weil_ai.langchain.create_agent`` inside a
    ``@weil_ai.agent`` factory. This fallback only wraps top-level invocation:
    it records ``langchain.agent.before``/``after`` and runs the whole
    invocation under the agent's wallet.
    """

    owner = _as_weil_agent(agent)
    inner = getattr(owner, "inner_agent", agent)
    if pensieve:
        _install_pensieve_tools(owner, inner)

    if getattr(inner, "__weil_langchain_wrapped__", False):
        return agent

    _wrap_invocations(owner, inner, audit=audit, verify=verify)

    setattr(inner, "__weil_langchain_wrapped__", True)
    return agent


def _wrap_invocations(owner: Any, inner: Any, *, audit: bool, verify: bool) -> None:
    if getattr(inner, "__weil_langchain_wrapped__", False):
        return
    for name in ("invoke", "stream"):
        if hasattr(inner, name):
            setattr(
                inner,
                name,
                _wrap_sync_invocation(owner, getattr(inner, name), audit, verify),
            )

    if hasattr(inner, "ainvoke"):
        setattr(
            inner,
            "ainvoke",
            _wrap_async_invocation(owner, inner.ainvoke, audit, verify),
        )

    # astream must stay a plain call that returns an async iterator: LangGraph's
    # own ainvoke runs ``async for chunk in self.astream(...)``.
    if hasattr(inner, "astream"):
        setattr(
            inner,
            "astream",
            _wrap_async_stream_invocation(owner, inner.astream, audit, verify),
        )

    setattr(inner, "__weil_langchain_wrapped__", True)


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
        trace = begin_trace(owner, root_from_call(args, kwargs))
        tokens = _set_langchain_wallet_context(owner, verify=verify)
        cleanup_deferred = False
        try:
            if audit:
                _run_sync_audit(owner, "langchain.invoke.before")
            result = fn(*args, **kwargs)
            if is_sync_stream(result):
                cleanup_deferred = True
                return _wrap_sync_stream(owner, result, trace, tokens, audit)
            if audit:
                _run_sync_audit(owner, "langchain.invoke.after")
            try:
                finish_trace_sync(trace)
            except Exception:
                pass
            return result
        finally:
            if not cleanup_deferred:
                reset_current_wallet_addr(tokens)
                try:
                    finish_trace_sync(trace)
                except Exception:
                    pass

    return wrapper


def _wrap_async_invocation(owner: Any, fn: Any, audit: bool, verify: bool) -> Any:
    async def wrapper(*args: Any, **kwargs: Any) -> Any:
        trace = begin_trace(owner, root_from_call(args, kwargs))
        tokens = _set_langchain_wallet_context(owner, verify=verify)
        cleanup_deferred = False
        try:
            if audit:
                await _audit(owner, "langchain.invoke.before")
            result = fn(*args, **kwargs)
            if inspect.isawaitable(result):
                result = await result
            if is_async_stream(result):
                cleanup_deferred = True
                return _wrap_async_stream(owner, result, trace, tokens, audit)
            if audit:
                await _audit(owner, "langchain.invoke.after")
            try:
                await finish_trace(trace)
            except Exception:
                pass
            return result
        finally:
            if not cleanup_deferred:
                reset_current_wallet_addr(tokens)
                try:
                    await finish_trace(trace)
                except Exception:
                    pass

    return wrapper


def _wrap_async_stream_invocation(owner: Any, fn: Any, audit: bool, verify: bool) -> Any:
    """Wrap ``astream`` so it returns an async iterator right away.

    Use a regular ``def`` because LangGraph uses ``async for`` directly on the
    result of ``astream``. An ``async def`` would return a coroutine, which cannot
    be iterated over this way.

    The stream starts running only when iteration begins. Set up the trace and
    wallet inside the iterator so they stay active while chunks and tool calls are
    being processed. Use ``finally`` to close the trace when iteration ends or the
    stream is closed.
    """
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        async def iterator() -> Any:
            trace = begin_trace(owner, root_from_call(args, kwargs))
            tokens = _set_langchain_wallet_context(owner, verify=verify)
            try:
                if audit:
                    await _audit(owner, "langchain.invoke.before")
                async for item in fn(*args, **kwargs):
                    yield item
                if audit:
                    await _audit(owner, "langchain.invoke.after")
            finally:
                reset_current_wallet_addr(tokens)
                try:
                    await finish_trace(trace)
                except Exception:
                    pass

        return iterator()

    return wrapper


def _wrap_sync_stream(owner: Any, result: Any, trace: Any, tokens: Any, audit: bool) -> Any:
    """Wrap a LangChain sync stream so cleanup happens after consumption.

    Streaming invocations keep wallet context and trace state alive until the
    iterator is exhausted or closed. Cleanup resets wallet context and finishes
    the trace in either case.
    """
    def iterator() -> Iterator[Any]:
        try:
            for item in result:
                yield item
            if audit:
                _run_sync_audit(owner, "langchain.invoke.after")
            try:
                finish_trace_sync(trace)
            except Exception:
                pass
        finally:
            reset_current_wallet_addr(tokens)
            try:
                finish_trace_sync(trace)
            except Exception:
                pass

    return iterator()


def _wrap_async_stream(owner: Any, result: Any, trace: Any, tokens: Any, audit: bool) -> Any:
    """Wrap a LangChain async stream so cleanup happens after consumption.

    This mirrors :func:`_wrap_sync_stream` for async iterators and defers the
    after-invocation audit record until the stream has completed normally.
    """
    async def iterator() -> Any:
        try:
            async for item in result:
                yield item
            if audit:
                await _audit(owner, "langchain.invoke.after")
            try:
                await finish_trace(trace)
            except Exception:
                pass
        finally:
            reset_current_wallet_addr(tokens)
            try:
                await finish_trace(trace)
            except Exception:
                pass

    return iterator()


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
        inner = getattr(owner, "inner_agent", owner)
        service = _secure_config(inner).get("service") if inner is not None else None
        payload = {
            "event": event,
            "ts": _now_iso(),
            **({"service": service} if service is not None else {}),
            **{k: v for k, v in fields.items() if v is not None},
        }
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


def _run_context() -> dict[str, Any]:
    """LangGraph agent name, best effort (empty outside a graph run)."""
    try:
        from langgraph.config import get_config

        config = get_config() or {}
    except Exception:
        return {}
    metadata = config.get("metadata") or {}
    return {
        "agent": metadata.get("lc_agent_name"),
    }


def _agent_context(owner: Any, state: Any, *, final: bool = False) -> dict[str, Any]:
    """Who ran, under which wallet, for whom: the LangChain twin of ADK's
    adk.agent.* fields. Metadata only; message text is never recorded."""
    context = _run_context()
    inner = getattr(owner, "inner_agent", owner)
    context.update(
        {
            "agent": context.get("agent") or getattr(inner, "name", None),
            "agent_type": type(inner).__name__,
            "wallet": _own_wallet(owner),
            "caller_wallet": _caller_wallet(owner),
            "state": _state_summary(state),
        }
    )
    if final and isinstance(state, dict):
        messages = state.get("messages") or []
        last = messages[-1] if messages else None
        context["tool_calls"] = sum(
            len(getattr(m, "tool_calls", None) or []) for m in messages
        )
        context["last_message_type"] = type(last).__name__ if last is not None else None
    return context


def _tool_context(owner: Any, request: Any) -> dict[str, Any]:
    return {
        **_run_context(),
        "tool": _tool_name(request),
        "tool_call_id": _tool_call_id(request),
        "wallet": _own_wallet(owner),
        "caller_wallet": _caller_wallet(owner),
    }


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


def _tool_args(request: Any) -> Any:
    call = getattr(request, "tool_call", None)
    if isinstance(call, dict):
        return call.get("args")
    return getattr(request, "args", None)


def _tool_args_with_metadata(request: Any, framework_call_id: Optional[str]) -> Any:
    args = _tool_args(request)
    if not framework_call_id:
        return args
    if isinstance(args, dict):
        return {
            **args,
            "_weil": {
                "framework": "langchain",
                "tool_call_id": framework_call_id,
            },
        }
    return {
        "value": args,
        "_weil": {
            "framework": "langchain",
            "tool_call_id": framework_call_id,
        },
    }


def _tool_result(result: Any) -> Any:
    return _decode_json(getattr(result, "content", result))


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


def to_secure_a2a(
    agent: Any,
    *,
    name: str,
    description: str,
    url: str,
    service: Optional[str] = None,
) -> Any:
    """Serve a Weil-wrapped agent over A2A, behind Weil's inbound gate.

    LangChain counterpart of ``WeilAgent.to_secure_a2a``. The wrapped object
    needs ``async run(text: str) -> str``. Callers must be signed (401) and,
    with *service*, hold a purpose on it (403). *url* is where the agent is
    reachable, as advertised in its card.
    """
    from a2a.helpers.proto_helpers import new_text_message
    from a2a.server.agent_execution import AgentExecutor
    from a2a.server.request_handlers import DefaultRequestHandler
    from a2a.server.routes import create_agent_card_routes, create_jsonrpc_routes
    from a2a.server.tasks import InMemoryTaskStore
    from a2a.types import AgentCapabilities, AgentCard, AgentInterface, AgentSkill
    from starlette.applications import Starlette

    from .adk import secure_a2a_app

    class Executor(AgentExecutor):
        async def execute(self, context: Any, event_queue: Any) -> None:
            reply = await agent.inner_agent.run(context.get_user_input())
            await event_queue.enqueue_event(new_text_message(reply or ""))

        async def cancel(self, context: Any, event_queue: Any) -> None:
            raise NotImplementedError

    card = AgentCard(
        name=name,
        description=description,
        version="1.0.0",
        supported_interfaces=[
            AgentInterface(
                url=url.rstrip("/") + "/", protocol_binding="JSONRPC", protocol_version="1.0"
            )
        ],
        capabilities=AgentCapabilities(),
        default_input_modes=["text/plain"],
        default_output_modes=["text/plain"],
        skills=[AgentSkill(id=name, name=name, description=description, tags=[name])],
    )
    handler = DefaultRequestHandler(
        agent_executor=Executor(), task_store=InMemoryTaskStore(), agent_card=card
    )
    app = Starlette(
        routes=[*create_agent_card_routes(card), *create_jsonrpc_routes(handler, "/")]
    )
    return secure_a2a_app(app, agent, service=service)
