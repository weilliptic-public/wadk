"""Run-scoped tool-call tracing for Weil agents.

The integration layers in :mod:`weil_ai.adk`, :mod:`weil_ai.langchain`, and
``WeilAgent`` use this module to create one trace per logical agent run,
append tool-call events as they happen, and publish the completed trace.
Tracing is best-effort: failures are recorded on the in-memory state and do
not interrupt the agent workflow.
"""

from __future__ import annotations

import contextlib
import contextvars
import inspect
import json
import uuid
from collections.abc import AsyncIterator, Iterator
from dataclasses import dataclass, field
from typing import Any, Optional

from weil_wallet import WeilClient
from weil_wallet.contract import ContractId
from weil_wallet.transaction import TransactionResult


@dataclass
class TraceState:
    """Mutable state for one active trace.

    ``trace_id`` identifies the on-chain trace record, ``root`` stores the
    serialized run prompt/input, and ``trace`` caches serialized tool events in
    memory. ``contract_id`` is resolved lazily and reused for later writes.
    """

    trace_id: str
    root: str
    trace: list[str] = field(default_factory=list)
    last_node_id: Optional[str] = None
    node_roots: dict[str, str] = field(default_factory=dict)
    last_child_by_parent: dict[str, str] = field(default_factory=dict)
    contract_id: Optional[ContractId] = None
    initialized: bool = False
    last_error: Optional[str] = None


@dataclass
class TraceHandle:
    """Caller-owned handle used to finish and clear a trace.

    ``active`` is true only for the outermost trace in the current context.
    Nested calls receive an inactive handle so they can call finish helpers
    without closing their parent's trace.
    """

    owner: Any
    token: Optional[contextvars.Token]
    active: bool
    handle_token: Optional[contextvars.Token] = None
    parent_token: Optional[contextvars.Token] = None


_CURRENT_TRACE: contextvars.ContextVar[Optional[TraceState]] = contextvars.ContextVar(
    "weil_ai_current_trace",
    default=None,
)
_CURRENT_HANDLE: contextvars.ContextVar[Optional[TraceHandle]] = contextvars.ContextVar(
    "weil_ai_current_trace_handle",
    default=None,
)
_CURRENT_PARENT_ID: contextvars.ContextVar[Optional[str]] = contextvars.ContextVar(
    "weil_ai_current_trace_parent_id",
    default=None,
)


def begin_trace(owner: Any, root: Any) -> TraceHandle:
    """Start a trace for the current logical run.

    Args:
        owner: Weil-wrapped agent that provides ``_get_client()`` for contract
            calls.
        root: User prompt or invocation payload. The value is normalized with
            :func:`serialize_root`.

    Returns:
        A :class:`TraceHandle`. If another trace is already active in this
        context, a child ``subtrace`` node is appended and the returned handle
        only restores that child parent context when finished.
    """
    current_state = _CURRENT_TRACE.get()
    if current_state is not None:
        nested_root = serialize_root(root)
        if nested_root == current_state.root:
            return TraceHandle(owner=owner, token=None, active=False)
        parent_id = _CURRENT_PARENT_ID.get()
        if parent_id is not None and current_state.node_roots.get(parent_id) == nested_root:
            return TraceHandle(owner=owner, token=None, active=False)
        node_id = str(uuid.uuid4())
        event = serialize_trace_node(
            "subtrace",
            {"root": nested_root},
            event_id=node_id,
            parent_id=parent_id,
        )
        current_state.node_roots[node_id] = nested_root
        current_state.trace.append(event)
        handle = current_handle()
        if handle is not None and current_state.initialized:
            try:
                build_trace_sync(handle.owner, current_state, event)
            except Exception as exc:
                current_state.last_error = f"{type(exc).__name__}: {exc}"
        return TraceHandle(
            owner=owner,
            token=None,
            active=False,
            parent_token=_CURRENT_PARENT_ID.set(node_id),
        )
    state = TraceState(trace_id=str(uuid.uuid4()), root=serialize_root(root))
    handle = TraceHandle(owner=owner, token=_CURRENT_TRACE.set(state), active=True)
    handle.handle_token = _CURRENT_HANDLE.set(handle)
    handle.parent_token = _CURRENT_PARENT_ID.set(None)
    try:
        initialize_trace_sync(owner, state)
    except Exception as exc:
        state.last_error = f"{type(exc).__name__}: {exc}"
    return handle


def current_handle() -> Optional[TraceHandle]:
    """Return the active trace handle for the current context, if any."""
    return _CURRENT_HANDLE.get()


def current_trace_id() -> Optional[str]:
    """Return the active trace's id, or None outside a traced run."""
    state = _CURRENT_TRACE.get()
    return state.trace_id if state is not None else None


def current_parent_id() -> Optional[str]:
    """Return the parent node id for new events in the current execution branch."""
    parent_id = _CURRENT_PARENT_ID.get()
    state = _CURRENT_TRACE.get()
    if state is None:
        return parent_id
    return state.last_child_by_parent.get(_parent_key(parent_id), parent_id)


def set_trace_parent(parent_id: Optional[str]) -> contextvars.Token:
    """Set the parent node id for subsequently recorded events."""
    return _CURRENT_PARENT_ID.set(parent_id)


def reset_trace_parent(token: contextvars.Token) -> None:
    """Restore the parent node id saved by :func:`set_trace_parent`."""
    _CURRENT_PARENT_ID.reset(token)


@contextlib.contextmanager
def trace_parent(parent_id: Optional[str]) -> Iterator[None]:
    """Temporarily attach new trace nodes to ``parent_id`` in this context.

    This is useful for frameworks that explicitly expose a sub-run or task id.
    Async tasks created inside this context inherit the parent id, so parallel
    work naturally records sibling nodes under the same parent.
    """
    token = set_trace_parent(parent_id)
    try:
        yield
    finally:
        reset_trace_parent(token)


async def finish_trace(handle: TraceHandle) -> Optional[TransactionResult]:
    """Publish the active trace and clear context-local state.

    Inactive handles, missing state, and traces that failed to initialize are
    ignored. The context variables are reset even if publishing fails.
    """
    if not handle.active or handle.token is None:
        if handle.parent_token is not None:
            _CURRENT_PARENT_ID.reset(handle.parent_token)
            handle.parent_token = None
        return None
    state = _CURRENT_TRACE.get()
    try:
        if state is None:
            return None
        if not state.initialized:
            return None
        return await _finish_initialized_trace(handle.owner, state)
    finally:
        handle.active = False
        if handle.handle_token is not None:
            _CURRENT_HANDLE.reset(handle.handle_token)
        if handle.parent_token is not None:
            _CURRENT_PARENT_ID.reset(handle.parent_token)
            handle.parent_token = None
        _CURRENT_TRACE.reset(handle.token)


def finish_trace_sync(handle: TraceHandle) -> Optional[TransactionResult]:
    """Synchronous bridge for finishing an initialized trace.

    This is used by sync agent integrations. It delegates async work through
    ``weil_ai.mcp._run_sync`` and always resets context-local trace state.
    """
    if not handle.active or handle.token is None:
        if handle.parent_token is not None:
            _CURRENT_PARENT_ID.reset(handle.parent_token)
            handle.parent_token = None
        return None
    from .mcp import _run_sync

    state = _CURRENT_TRACE.get()
    try:
        if state is None:
            return None
        if not state.initialized:
            return None
        return _run_sync(_finish_initialized_trace(handle.owner, state))
    finally:
        handle.active = False
        if handle.handle_token is not None:
            _CURRENT_HANDLE.reset(handle.handle_token)
        if handle.parent_token is not None:
            _CURRENT_PARENT_ID.reset(handle.parent_token)
            handle.parent_token = None
        _CURRENT_TRACE.reset(handle.token)


async def arecord_tool_call(
    name: Any,
    args: Any = None,
    result: Any = None,
    *,
    event_id: Any = None,
    parent_id: Any = None,
) -> None:
    """Append one async tool-call event to the active trace.

    This mirrors :func:`record_tool_call` but awaits the incremental contract
    write.
    """
    state = _CURRENT_TRACE.get()
    if state is None:
        return
    event = serialize_tool_call(
        name,
        args,
        result,
        event_id=event_id,
        parent_id=parent_id,
    )
    _advance_trace_parent(state, event)
    state.trace.append(event)
    if not state.initialized:
        return
    handle = current_handle()
    if handle is None:
        return
    try:
        await build_trace(handle.owner, state, event)
    except Exception as exc:
        state.last_error = f"{type(exc).__name__}: {exc}"


def record_tool_call(
    name: Any,
    args: Any = None,
    result: Any = None,
    *,
    event_id: Any = None,
    parent_id: Any = None,
) -> None:
    """Append one sync tool-call event to the active trace.

    The event is stored in memory and, once the trace is initialized, appended
    to the tracer contract with ``build_trace``. Errors are captured on
    ``TraceState.last_error`` and are not raised.
    """
    state = _CURRENT_TRACE.get()
    if state is None:
        return
    event = serialize_tool_call(
        name,
        args,
        result,
        event_id=event_id,
        parent_id=parent_id,
    )
    _advance_trace_parent(state, event)
    state.trace.append(event)
    if not state.initialized:
        return
    handle = current_handle()
    if handle is None:
        return
    try:
        build_trace_sync(handle.owner, state, event)
    except Exception as exc:
        state.last_error = f"{type(exc).__name__}: {exc}"


def serialize_tool_call(
    name: Any,
    args: Any = None,
    result: Any = None,
    *,
    event_id: Any = None,
    parent_id: Any = None,
) -> str:
    """Serialize a tool-call trace node as stable compact JSON.

    Non-JSON-native values are converted through model/dict helpers when
    possible and otherwise represented with ``repr``. Each node includes
    ``id`` and ``parent_id`` so consumers can reconstruct branching execution
    from the contract's ordered ``list<TraceEvent>`` storage.
    """
    content = {
        "tool": str(name) if name is not None else None,
        "args": _jsonable(args),
        "result": _jsonable(result),
    }
    return serialize_trace_node(
        "tool_call",
        content,
        event_id=event_id,
        parent_id=parent_id,
        legacy_fields=content,
    )


def serialize_trace_node(
    node_type: str,
    content: Any,
    *,
    event_id: Any = None,
    parent_id: Any = None,
    legacy_fields: Optional[dict[str, Any]] = None,
) -> str:
    """Serialize a generic graph trace node as stable compact JSON."""
    node_id = str(event_id) if event_id is not None else str(uuid.uuid4())
    resolved_parent = parent_id if parent_id is not None else _CURRENT_PARENT_ID.get()
    event = {
        "id": node_id,
        "parent_id": str(resolved_parent) if resolved_parent is not None else None,
        "type": node_type,
        "content": _jsonable(content),
    }
    if legacy_fields:
        event.update(legacy_fields)
    return json.dumps(event, sort_keys=True, separators=(",", ":"), default=repr)


def contract_trace_event(event: str) -> dict[str, Any]:
    """Convert an in-memory JSON node into the backend ``TraceEvent`` record."""
    try:
        parsed = json.loads(event)
    except ValueError:
        parsed = {"content": event}
    if not isinstance(parsed, dict):
        parsed = {"content": parsed}

    node_id = parsed.get("id") or str(uuid.uuid4())
    parent_id = parsed.get("parent_id")
    event_type = parsed.get("type") or parsed.get("event_type") or "event"
    content = parsed.get("content", parsed)
    return {
        "id": str(node_id),
        "parent_id": str(parent_id) if parent_id is not None else None,
        "event_type": str(event_type),
        "content": json.dumps(_jsonable(content), sort_keys=True, default=repr),
    }


def _advance_trace_parent(state: TraceState, event: str) -> None:
    try:
        parsed = json.loads(event)
    except ValueError:
        return
    node_id = parsed.get("id") if isinstance(parsed, dict) else None
    if node_id is None:
        return
    parent_id = parsed.get("parent_id") if isinstance(parsed, dict) else None
    state.last_child_by_parent[_parent_key(parent_id)] = str(node_id)
    state.last_node_id = str(node_id)
    _CURRENT_PARENT_ID.set(str(node_id))


def _parent_key(parent_id: Any) -> str:
    return "__root__" if parent_id is None else str(parent_id)


async def initialize_trace(owner: Any, state: TraceState) -> TransactionResult:
    """Create the trace record in the tracer contract.

    The trace starts with the run root and an empty event list. On success,
    ``state.initialized`` is set so future tool events can be appended
    incrementally.
    """
    contract_id = await _tracer_contract_id(owner, state)
    result = await _execute_tracer(
        owner,
        state,
        "add_trace",
        {
            "trace_id": state.trace_id,
            "root": state.root,
            "trace": [],
        },
    )
    state.initialized = True
    state.contract_id = contract_id
    return result


def initialize_trace_sync(owner: Any, state: TraceState) -> Optional[TransactionResult]:
    """Synchronous bridge for :func:`initialize_trace`."""
    from .mcp import _run_sync

    return _run_sync(initialize_trace(owner, state))


async def build_trace(
    owner: Any,
    state: TraceState,
    event: str,
) -> TransactionResult:
    """Append one serialized event to an existing trace contract record."""
    return await _execute_tracer(
        owner,
        state,
        "build_trace",
        {
            "trace_id": state.trace_id,
            "event": contract_trace_event(event),
        },
    )


def build_trace_sync(
    owner: Any,
    state: TraceState,
    event: str,
) -> Optional[TransactionResult]:
    """Synchronous bridge for :func:`build_trace`."""
    from .mcp import _run_sync

    return _run_sync(build_trace(owner, state, event))


async def get_trace(owner: Any, state: TraceState) -> Any:
    """Fetch and decode the trace payload from the tracer contract."""
    result = await _execute_tracer(
        owner,
        state,
        "get_trace",
        {"trace_id": state.trace_id},
    )
    return _parse_contract_result(result.txn_result)


async def index_trace(owner: Any, state: TraceState) -> TransactionResult:
    """Ask the tracer contract to index the completed trace.

    The contract call is non-blocking because indexing is follow-up work and
    should not delay the agent response path.
    """
    return await _execute_tracer(
        owner,
        state,
        "index_trace",
        {"trace_id": state.trace_id},
        is_non_blocking=True,
    )


async def publish_trace(owner: Any, trace: Any) -> TransactionResult:
    """Publish a completed trace to the organization audit stream."""
    client = owner._get_client()
    org_name = _wallet_org_name(client)
    contract_id = await client._get_publisher_contract_id(f"publisher::{org_name}")
    method_args = json.dumps(
        {
            "topic": "audits",
            "payload": json.dumps(_jsonable(trace), default=repr),
        }
    )
    return await client.to_contract_client(contract_id).execute(
        "publish",
        method_args,
        False,
        True,
    )


async def _finish_initialized_trace(owner: Any, state: TraceState) -> TransactionResult:
    trace = await get_trace(owner, state)
    try:
        await index_trace(owner, state)
    except Exception as exc:
        state.last_error = f"{type(exc).__name__}: {exc}"
    return await publish_trace(owner, trace)


async def submit_trace(owner: Any, state: TraceState) -> TransactionResult:
    """Initialize a trace and append events already present in ``state``.

    This is a compatibility helper for callers that build ``TraceState.trace``
    before initializing the contract-backed trace.
    """
    result = await initialize_trace(owner, state)
    for event in state.trace:
        result = await build_trace(owner, state, event)
    return result


async def _execute_tracer(
    owner: Any,
    state: TraceState,
    method_name: str,
    method_args: dict[str, Any],
    *,
    is_non_blocking: bool = False,
) -> TransactionResult:
    contract_id = await _tracer_contract_id(owner, state)
    return await owner._get_client().to_contract_client(contract_id).execute(
        method_name,
        json.dumps(method_args, default=repr),
        False,
        is_non_blocking,
    )


async def _tracer_contract_id(owner: Any, state: TraceState) -> ContractId:
    if state.contract_id is not None:
        return state.contract_id
    client = owner._get_client()
    state.contract_id = await WeilClient.get_applet_id_for_name(
        client._http_client,
        f"weilchain::weil",
        wallet_address=client.wallet_addr(),
    )
    return state.contract_id


def _wallet_org_name(client: Any) -> str:
    org = getattr(getattr(client, "_wallet", None), "org", None)
    org_name = getattr(org, "name", None)
    if not org_name:
        raise RuntimeError("tool tracing requires a wallet linked to an organization")
    return org_name


def _parse_contract_result(value: str) -> Any:
    parsed: Any = value
    for _ in range(3):
        if isinstance(parsed, str):
            try:
                parsed = json.loads(parsed)
            except ValueError:
                return parsed
            continue
        if isinstance(parsed, dict) and "Err" in parsed:
            raise RuntimeError(f"tracer returned an error: {parsed['Err']}")
        if isinstance(parsed, dict) and "Ok" in parsed:
            parsed = parsed["Ok"]
            continue
        return parsed
    return parsed


def serialize_root(value: Any) -> str:
    """Serialize an invocation root for storage on the trace.

    Prompt-like values are preferred so traces are easy to search. If no prompt
    can be extracted, the full value is converted to a stable string.
    """
    prompt = extract_prompt(value)
    if prompt is not None:
        return prompt
    return _to_string(value)


def root_from_call(args: tuple[Any, ...], kwargs: dict[str, Any]) -> Any:
    """Choose the best root payload from an invocation's args and kwargs."""
    for key in ("input", "prompt", "message", "messages", "query"):
        if key in kwargs:
            return kwargs[key]
    if args:
        return args[0]
    return kwargs


def extract_prompt(value: Any) -> Optional[str]:
    """Extract human/user prompt text from common agent invocation shapes.

    Supports plain strings, dictionaries with prompt-like keys, chat message
    lists, message objects, and objects exposing prompt/input/content-style
    attributes.
    """
    if value is None:
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        for key in ("prompt", "input", "query"):
            prompt = extract_prompt(value.get(key))
            if prompt:
                return prompt
        messages = value.get("messages")
        prompt = _prompt_from_messages(messages)
        if prompt:
            return prompt
        content = value.get("content")
        if isinstance(content, str):
            return content
    prompt = _prompt_from_messages(value)
    if prompt:
        return prompt
    for attr in ("prompt", "input", "query", "user_content", "content"):
        try:
            prompt = extract_prompt(getattr(value, attr))
        except Exception:
            continue
        if prompt:
            return prompt
    return None


def wrap_sync_iterator(result: Any, handle: TraceHandle) -> Iterator[Any]:
    """Yield a sync stream and finish its trace when iteration ends."""
    try:
        for item in result:
            yield item
    finally:
        try:
            finish_trace_sync(handle)
        except Exception:
            pass


async def wrap_async_iterator(result: Any, handle: TraceHandle) -> AsyncIterator[Any]:
    """Yield an async stream and finish its trace when iteration ends."""
    try:
        async for item in result:
            yield item
    finally:
        try:
            await finish_trace(handle)
        except Exception:
            pass


def _prompt_from_messages(messages: Any) -> Optional[str]:
    if not isinstance(messages, (list, tuple)):
        return None
    fallback: Optional[str] = None
    for message in messages:
        role = None
        content = None
        if isinstance(message, dict):
            role = message.get("role") or message.get("type")
            content = message.get("content")
        else:
            role = getattr(message, "role", None) or getattr(message, "type", None)
            content = getattr(message, "content", None)
        text = _content_text(content)
        if not text:
            continue
        fallback = fallback or text
        if role in ("user", "human"):
            return text
    return fallback


def _content_text(content: Any) -> Optional[str]:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, dict):
                text = item.get("text")
                if isinstance(text, str):
                    parts.append(text)
        if parts:
            return "\n".join(parts)
    return None


def _jsonable(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, dict):
        return {str(key): _jsonable(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_jsonable(item) for item in value]
    if hasattr(value, "model_dump"):
        try:
            return value.model_dump(mode="json", exclude_none=True)
        except Exception:
            pass
    if hasattr(value, "dict"):
        try:
            return value.dict()
        except Exception:
            pass
    return repr(value)


def _to_string(value: Any) -> str:
    try:
        return json.dumps(_jsonable(value), sort_keys=True, default=repr)
    except Exception:
        return repr(value)


def is_sync_stream(result: Any) -> bool:
    """Return true when ``result`` looks like a sync stream/generator."""
    return inspect.isgenerator(result) or (
        hasattr(result, "__iter__")
        and not isinstance(result, (str, bytes, bytearray, dict, list, tuple))
    )


def is_async_stream(result: Any) -> bool:
    """Return true when ``result`` looks like an async stream/generator."""
    return hasattr(result, "__aiter__")
