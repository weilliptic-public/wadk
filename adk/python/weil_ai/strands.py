"""Strands Agents integration for Weil identity, secured tools, audit and traces.

Strands has a native hook registry, so unlike the ADK integration nothing is
monkey-patched: :class:`WeilStrandsHooks` is a Strands ``HookProvider`` that
registers callbacks on the agent's lifecycle events.

- ``BeforeInvocationEvent`` / ``AfterInvocationEvent``: start and publish the
  run trace, and audit the run.
- ``BeforeToolCallEvent`` / ``AfterToolCallEvent``: run ``@secured`` tools
  under the agent's wallet, audit them with their arguments and response, and
  add every tool call to the run trace.

Use :func:`create_agent` inside a ``@weil_ai.agent(...)`` factory, or
``agent.setup_secure_strands()`` for an agent that already exists. Either one
also secures the agent's sub-agents: agents used as tools, and the nodes of a
Graph or Swarm.

A2A: :func:`to_secure_a2a` serves an agent behind ``WeilA2AMiddleware`` (the
caller's signature and permission are checked before the agent runs), and
:func:`remote_agent` / :func:`remote_agent_tool` call a remote agent with
every request signed by the calling agent's wallet.
"""

from __future__ import annotations

import json
from typing import Any, Callable, Optional, Union

import httpx

from .adk import (
    WeilA2AMiddleware,
    _audit,
    _caller_wallet,
    _has_own_identity,
    _message_context,
    _own_wallet,
    _short_hash,
    _signed_payload_from_http_body,
)
from .mcp import _secured_tool_info, reset_current_wallet_addr, set_current_wallet_addr
from .memory import append_unique_tools, make_strands_pensieve_tools
from .tracer import arecord_tool_call, begin_trace, finish_trace


# HookOrder.SDK_FIRST / SDK_LAST, kept as numbers so this module imports
# without Strands installed.
_FIRST = -100
_LAST = 100
# The WeilStrandsHooks installed on a Strands agent, set when they are.
_SECURED_FLAG = "__weil_strands_hooks__"
_DEFAULT_TIMEOUT = 300


def create_agent(
    *args: Any,
    hooks: Union[tuple[Any, ...], list[Any]] = (),
    audit: bool = True,
    verify: bool = True,
    pensieve: bool = True,
    **kwargs: Any,
) -> Any:
    """Create a Strands ``Agent`` with Weil hooks installed.

    Use this inside a ``@weil_ai.agent(...)`` factory. Arguments are passed to
    Strands' ``Agent(...)`` unchanged; Weil's hooks are added first, and the
    Pensieve memory tools are added to ``tools`` unless ``pensieve=False``.
    Agents passed in ``tools`` are secured as sub-agents.
    """
    try:
        from strands import Agent
    except ImportError as exc:
        raise ImportError(
            "strands-agents is required for weil_ai.strands.create_agent(); "
            "install weil-ai[strands]"
        ) from exc

    owner = _current_identity()
    if pensieve:
        kwargs["tools"] = append_unique_tools(
            kwargs.get("tools"),
            make_strands_pensieve_tools(owner),
        )
    weil_hooks = WeilStrandsHooks(owner, audit=audit, verify=verify)
    agent = Agent(*args, hooks=[weil_hooks, *list(hooks or [])], **kwargs)
    setattr(agent, _SECURED_FLAG, weil_hooks)
    _secure_sub_agents(owner, agent, audit=audit, verify=verify)
    return agent


def setup_secure_strands(
    agent: Any,
    *,
    audit: bool = True,
    verify: bool = True,
    pensieve: bool = True,
) -> Any:
    """Install Weil hooks on an already-created Strands agent, Graph or Swarm.

    ``agent`` is the ``WeilAgent`` wrapping it. Sub-agents are secured too: an
    agent used as a tool, or a Graph or Swarm node, acts under its own wallet
    when it was wrapped with ``weil_ai.agent``, and under this agent's
    otherwise; remote A2A nodes are signed with this agent's wallet. An agent
    that already has Weil's hooks is left as it is.
    """
    owner = _as_weil_agent(agent)
    inner = getattr(owner, "inner_agent", agent)
    if pensieve and _is_local_agent(inner) and getattr(inner, _SECURED_FLAG, None) is None:
        _install_pensieve_tools(owner, inner)
    _secure_node(owner, inner, audit=audit, verify=verify)
    return agent


class WeilStrandsHooks:
    """Strands ``HookProvider`` that audits and traces runs under a Weil wallet.

    The run trace is started before any other hook sees the run and published
    after every other hook has finished with it, so the trace and the audit
    record what the model actually received after other hooks rewrote it.

    ``parent`` is the agent that runs this one as a sub-agent, when that is a
    different wallet; its wallet is recorded as the caller.
    """

    def __init__(
        self,
        owner: Any,
        *,
        audit: bool = True,
        verify: bool = True,
        parent: Any = None,
    ) -> None:
        self._owner = owner
        self._audit = audit
        self._verify = verify
        self._parent = parent
        # Trace handles for runs in flight, keyed by (agent, invocation state)
        # so concurrent runs, and agents sharing one invocation state in a
        # multi-agent graph, never finish each other's trace.
        self._traces: dict[tuple[int, int], Any] = {}
        # Wallet context tokens for @secured tool calls in flight, by toolUseId.
        self._tokens: dict[str, Any] = {}

    def register_hooks(self, registry: Any, **kwargs: Any) -> None:
        from strands.hooks import (
            AfterInvocationEvent,
            AfterToolCallEvent,
            BeforeInvocationEvent,
            BeforeToolCallEvent,
        )

        registry.add_callback(BeforeInvocationEvent, self.before_invocation, order=_FIRST)
        registry.add_callback(AfterInvocationEvent, self.after_invocation, order=_LAST)
        registry.add_callback(BeforeToolCallEvent, self.before_tool, order=_FIRST)
        registry.add_callback(AfterToolCallEvent, self.after_tool, order=_LAST)

    async def before_invocation(self, event: Any) -> None:
        trace = begin_trace(self._owner, event.messages)
        self._traces[_run_key(event)] = trace
        if self._audit:
            await _audit(self._owner, "strands.agent.before", **self._agent_context(event))

    async def after_invocation(self, event: Any) -> None:
        try:
            if self._audit:
                await _audit(self._owner, "strands.agent.after", **self._agent_context(event))
        finally:
            trace = self._traces.pop(_run_key(event), None)
            if trace is not None:
                try:
                    await finish_trace(trace)
                except Exception:
                    pass

    async def before_tool(self, event: Any) -> None:
        secured = _secured_tool_info(event.selected_tool)
        if secured is None:
            return
        if self._audit:
            await _audit(
                self._owner,
                "strands.tool.before",
                **self._tool_context(event, secured),
                args=event.tool_use.get("input"),
            )
        # A secured tool authorizes as the acting agent. The token is kept
        # until the matching after-tool callback, paired by toolUseId.
        self._tokens[_tool_use_id(event)] = set_current_wallet_addr(
            self._owner._wallet_auth_address(),
            mode="strands",
            sentinel_host=self._owner._sentinel_host,
            verify=self._owner._verify if self._verify else False,
        )

    async def after_tool(self, event: Any) -> None:
        if tokens := self._tokens.pop(_tool_use_id(event), None):
            reset_current_wallet_addr(tokens)
        response = _tool_result(event.result)
        await arecord_tool_call(event.tool_use.get("name"), event.tool_use.get("input"), response)
        # A cancelled call has no selected tool; look at the tool it was for.
        secured = _secured_tool_info(event.selected_tool) or _secured_tool_info(
            _lookup_tool(event.agent, event.tool_use.get("name"))
        )
        if self._audit and secured is not None:
            await _audit(
                self._owner,
                "strands.tool.after",
                **self._tool_context(event, secured),
                status=event.result.get("status"),
                response=response,
            )

    def _agent_context(self, event: Any) -> dict[str, Any]:
        agent = event.agent
        return {
            "agent": getattr(agent, "name", None) or None,
            "agent_id": getattr(agent, "agent_id", None),
            "agent_type": type(agent).__name__,
            "wallet": _own_wallet(self._owner),
            # The enclosing agent for a sub-agent; otherwise the caller of a
            # signed A2A request this run is serving, if any.
            "caller_wallet": _caller_wallet(self._owner, self._parent),
        }

    def _tool_context(self, event: Any, secured: dict[str, str]) -> dict[str, Any]:
        return {
            "agent": getattr(event.agent, "name", None) or None,
            "tool": event.tool_use.get("name"),
            "tool_call_id": _tool_use_id(event),
            **secured,
            "wallet": _own_wallet(self._owner),
            "caller_wallet": _caller_wallet(self._owner, self._parent),
        }


# ----------------------------------------------------------------------
# Sub-agents
# ----------------------------------------------------------------------


def _secure_node(
    acting: Any,
    node: Any,
    *,
    audit: bool,
    verify: bool,
    parent: Any = None,
) -> None:
    """Secure one agent, Graph or Swarm node under *acting*'s wallet, recursively."""
    if _is_remote_agent(node):
        _sign_remote_agent(acting, node, audit=audit)
    elif _is_local_agent(node):
        installed = getattr(node, _SECURED_FLAG, None)
        if installed is None:
            weil_hooks = WeilStrandsHooks(acting, audit=audit, verify=verify, parent=parent)
            node.hooks.add_hook(weil_hooks)
            setattr(node, _SECURED_FLAG, weil_hooks)
        elif parent is not None and installed._parent is None:
            # Built with its own hooks (create_agent); now that it is known to
            # run under another agent, record that agent as its caller.
            installed._parent = parent
        _secure_sub_agents(acting, node, audit=audit, verify=verify)
    else:
        for executor in _node_executors(node):
            _secure_child(acting, executor, audit=audit, verify=verify)


def _secure_sub_agents(acting: Any, agent: Any, *, audit: bool, verify: bool) -> None:
    """Secure the agents *agent* runs as tools."""
    for tool in list(getattr(agent, "tool_registry", None).registry.values()):
        sub = getattr(tool, "agent", None)
        if sub is not None and (_is_local_agent(sub) or _is_remote_agent(sub)):
            _secure_child(acting, sub, audit=audit, verify=verify)


def _secure_child(acting: Any, child: Any, *, audit: bool, verify: bool) -> None:
    # Strands holds the unwrapped agent; recover its WeilAgent, and wallet.
    proxy = getattr(child, "__weil_agent__", None)
    if proxy is not None and _has_own_identity(proxy):
        # Its own wallet signs its audits and authorizes its tools; the
        # enclosing agent is recorded as the caller.
        _secure_node(proxy, child, audit=audit, verify=verify, parent=acting)
    else:
        _secure_node(acting, child, audit=audit, verify=verify)


def _node_executors(node: Any) -> list[Any]:
    """The agents a Graph or Swarm runs, in node order."""
    nodes = getattr(node, "nodes", None)
    if isinstance(nodes, dict):
        nodes = nodes.values()
    return [getattr(n, "executor", None) for n in nodes or [] if getattr(n, "executor", None)]


def _is_local_agent(value: Any) -> bool:
    return hasattr(value, "hooks") and hasattr(value, "tool_registry")


def _is_remote_agent(value: Any) -> bool:
    return hasattr(value, "endpoint") and hasattr(value, "_client_config")


# ----------------------------------------------------------------------
# A2A: outbound
# ----------------------------------------------------------------------


def remote_agent(
    endpoint: str,
    *,
    owner: Any = None,
    audit: bool = True,
    timeout: int = _DEFAULT_TIMEOUT,
    **kwargs: Any,
) -> Any:
    """Return a Strands ``A2AAgent`` whose every request *owner*'s wallet signs.

    *owner* defaults to the agent being built in the current
    ``@weil_ai.agent(...)`` factory. Other arguments are passed to
    ``A2AAgent``. The remote side checks the signature with
    ``WeilA2AMiddleware`` (see :func:`to_secure_a2a`).
    """
    try:
        from strands.agent.a2a_agent import A2AAgent
    except ImportError as exc:
        raise ImportError(
            "strands-agents[a2a] is required for weil_ai.strands.remote_agent()"
        ) from exc

    agent = A2AAgent(endpoint, timeout=timeout, **kwargs)
    _sign_remote_agent(owner or _current_identity(), agent, audit=audit)
    return agent


def remote_agent_tool(
    endpoint: str,
    *,
    name: str,
    description: str,
    owner: Any = None,
    audit: bool = True,
    timeout: int = _DEFAULT_TIMEOUT,
) -> Any:
    """Return a Strands tool that asks the remote A2A agent at *endpoint*.

    The tool takes one ``request`` string and returns the remote agent's
    answer. Requests are signed as in :func:`remote_agent`; a refusal (HTTP 401
    or 403) reaches the model as the tool's error.
    """
    from strands import tool

    agent = remote_agent(endpoint, owner=owner, audit=audit, timeout=timeout, name=name)

    async def ask(request: str) -> str:
        """Ask the remote agent.

        Args:
            request: What to ask it, in plain language, with every detail it needs.
        """
        return str(await agent.invoke_async(request))

    return tool(name=name, description=description)(ask)


def _sign_remote_agent(owner: Any, agent: Any, *, audit: bool) -> None:
    """Route an ``A2AAgent``'s requests through a client signing with *owner*."""
    if getattr(agent, "_a2a_client_factory", None) is not None:
        return  # its own deprecated factory builds the clients; leave it
    config = getattr(agent, "_client_config", None)
    existing = getattr(config, "httpx_client", None)
    if isinstance(getattr(existing, "_transport", None), _SigningTransport):
        return
    if existing is not None:
        raise ValueError(
            f"A2A agent {getattr(agent, 'name', None) or agent.endpoint!r} already has "
            "its own httpx client; let weil_ai.strands build it so requests are signed"
        )
    from a2a.client import ClientConfig

    transport = _SigningTransport(
        owner,
        audit=audit,
        target=getattr(agent, "name", None),
        target_url=agent.endpoint,
    )
    client = httpx.AsyncClient(transport=transport, timeout=agent.timeout)
    if config is None:
        agent._client_config = ClientConfig(httpx_client=client)
    else:
        config.httpx_client = client


class _SigningTransport(httpx.AsyncBaseTransport):
    """httpx transport that signs each A2A POST with a Weil wallet.

    The signature covers the request's A2A message, extracted exactly as
    ``WeilA2AMiddleware`` extracts it on the other side.

    Each request opens its own transport on the caller's event loop, closed
    when its response is. A pooled connection belongs to the loop that opened
    it, Strands runs each sync call on a fresh loop, and an A2A response
    streams back to the caller, so no connection is kept between requests.
    """

    def __init__(
        self,
        owner: Any,
        *,
        audit: bool = True,
        target: Optional[str] = None,
        target_url: Optional[str] = None,
        transport_factory: Callable[[], httpx.AsyncBaseTransport] = httpx.AsyncHTTPTransport,
    ) -> None:
        self._owner = owner
        self._audit = audit
        self._target = {"target": target, "target_url": target_url}
        self._transport_factory = transport_factory

    async def _send(self, request: httpx.Request) -> httpx.Response:
        """Send *request* on a transport of its own, closed with the response."""
        transport = self._transport_factory()
        try:
            response = await transport.handle_async_request(request)
        except BaseException:
            await transport.aclose()
            raise
        response.stream = _ClosingStream(response.stream, transport)
        return response

    async def handle_async_request(self, request: httpx.Request) -> httpx.Response:
        if request.method != "POST":
            return await self._send(request)

        payload = _signed_payload_from_http_body(await request.aread())
        headers = await self._owner.aget_auth_headers(body=payload)
        request.headers.update(headers)
        message = _message_context(payload)
        if self._audit:
            await _audit(
                self._owner,
                "a2a.request.signed",
                direction="outbound",
                wallet=_own_wallet(self._owner),
                signed_body_sha256=_short_hash(headers.get("X-Body-SHA256")),
                **self._target,
                **message,
            )
        response = await self._send(request)
        if self._audit:
            await _audit(
                self._owner,
                "a2a.response.received",
                direction="inbound",
                wallet=_own_wallet(self._owner),
                status=response.status_code,
                outcome="accepted" if response.status_code < 400 else "rejected",
                **self._target,
                **message,
            )
        return response


class _ClosingStream(httpx.AsyncByteStream):
    """A response body that closes its request's own transport when closed."""

    def __init__(self, stream: Any, transport: httpx.AsyncBaseTransport) -> None:
        self._stream = stream
        self._transport = transport

    async def __aiter__(self) -> Any:
        async for chunk in self._stream:
            yield chunk

    async def aclose(self) -> None:
        try:
            await self._stream.aclose()
        finally:
            await self._transport.aclose()


# ----------------------------------------------------------------------
# A2A: inbound
# ----------------------------------------------------------------------


def to_secure_a2a(
    agent: Any,
    *,
    service: Optional[str] = None,
    audit: bool = True,
    verify_signature: bool = True,
    **server_kwargs: Any,
) -> Any:
    """Serve a Strands agent over A2A behind ``WeilA2AMiddleware``.

    *agent* is a ``@weil_ai.agent`` factory, called once per A2A conversation
    for a fresh agent (Strands' recommended ``agent_factory``), or a single
    ``WeilAgent``. The middleware rejects unsigned requests, checks the
    caller's wallet holds Execution or Management on *service* when it is
    given, audits each request, and records the caller's wallet for the run.
    Other arguments go to Strands' ``A2AServer`` (``host``, ``port`` ...).

    Returns the Starlette app; serve it with uvicorn.
    """
    try:
        from strands.multiagent.a2a import A2AServer
    except ImportError as exc:
        raise ImportError(
            "strands-agents[a2a] is required for weil_ai.strands.to_secure_a2a()"
        ) from exc

    # The A2A-spec stream; Strands' default stream is its own, older format.
    server_kwargs.setdefault("enable_a2a_compliant_streaming", True)
    if _has_own_identity(agent):
        owner = agent
        if getattr(owner.inner_agent, _SECURED_FLAG, None) is None:
            setup_secure_strands(owner, audit=audit)
        server = A2AServer(agent=owner.inner_agent, **server_kwargs)
    else:
        factory = agent
        owner = _secured(factory(), audit=audit)
        server = A2AServer(
            agent_factory=lambda context_id: _secured(factory(), audit=audit).inner_agent,
            **server_kwargs,
        )
    return secure_a2a_app(
        server.to_starlette_app(),
        owner,
        service=service,
        audit=audit,
        verify_signature=verify_signature,
    )


def secure_a2a_app(
    app: Any,
    agent: Any,
    *,
    service: Optional[str] = None,
    audit: bool = True,
    verify_signature: bool = True,
) -> Any:
    """Add inbound Weil verification middleware to a Starlette A2A app."""
    app.add_middleware(
        WeilA2AMiddleware,
        agent=_as_weil_agent(agent),
        service=service,
        audit=audit,
        verify_signature=verify_signature,
    )
    return app


def _secured(agent: Any, *, audit: bool) -> Any:
    owner = _as_weil_agent(agent)
    if getattr(owner.inner_agent, _SECURED_FLAG, None) is None:
        setup_secure_strands(owner, audit=audit)
    return owner


# ----------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------


def _run_key(event: Any) -> tuple[int, int]:
    return id(event.agent), id(event.invocation_state)


def _tool_use_id(event: Any) -> str:
    return str(event.tool_use.get("toolUseId"))


def _lookup_tool(agent: Any, name: Optional[str]) -> Any:
    try:
        return agent.tool_registry.registry.get(name)
    except Exception:
        return None


def _tool_result(result: Any) -> Any:
    """The value a Strands ToolResult carries, as the model receives it.

    A single text block is decoded when it holds JSON; an error result is
    recorded as ``{"error": ...}``.
    """
    if not isinstance(result, dict):
        return result
    values = []
    for block in result.get("content") or []:
        if "json" in block:
            values.append(block["json"])
        elif "text" in block:
            values.append(_decode_json(block["text"]))
        else:
            values.append(block)
    value = values[0] if len(values) == 1 else values
    if result.get("status") == "error":
        return {"error": value}
    return value


def _decode_json(value: str) -> Any:
    try:
        return json.loads(value)
    except ValueError:
        return value


def _install_pensieve_tools(owner: Any, agent: Any) -> None:
    registry = getattr(agent, "tool_registry", None)
    if registry is None:
        return
    existing = set(registry.registry)
    tools = [
        tool for tool in make_strands_pensieve_tools(owner) if tool.tool_name not in existing
    ]
    if tools:
        registry.process_tools(tools)


def _current_identity() -> Any:
    from .langchain import _FACTORY_IDENTITY

    owner = _FACTORY_IDENTITY.get()
    if owner is None:
        raise RuntimeError(
            "No active Weil identity. Use weil_ai.strands.create_agent(...) "
            "inside a @weil_ai.agent(...) factory, or pass owner=."
        )
    return owner


def _as_weil_agent(agent: Any) -> Any:
    if _has_own_identity(agent):
        return agent
    raise TypeError("weil_ai.strands requires an object wrapped with weil_ai.agent")
