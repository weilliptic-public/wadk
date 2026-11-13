"""WeilAgent: attach a Weil wallet identity to any agent object.

Wraps any agent (LangChain, CrewAI, OpenAI Agents SDK, AutoGen, LlamaIndex, …)
with a Weil wallet so the agent gains two capabilities on top of its unchanged
interface:

  agent.get_auth_headers()  → dict
      Signed X-Wallet-Address / X-Signature / X-Message / X-Timestamp headers
      ready to pass to an MCP transport (e.g. streamablehttp_client).

  agent.audit(log)          → TransactionResult
      Write a log entry to the on-chain auditor applet.

Every other attribute/method call is transparently forwarded to the wrapped
agent, so agent.run(), agent.invoke(), agent.ainvoke(), etc. work unchanged.

Usage::

    # Option A: wrap an existing agent instance
    agent = WeilAgent(raw_agent, private_key_path="private_key.wc")
    headers = agent.get_auth_headers()
    agent.invoke({"messages": [...]})
    agent.audit("invoked agent")

    # Option B: decorator on a factory function
    @weil_agent
    def create_agent():
        return MyAgent()

    agent = create_agent()          # wallet auto-discovered from private_key.wc
    agent.get_auth_headers()
"""

from __future__ import annotations

import functools
import inspect
import json
import os
import warnings
import httpx
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Callable, Iterator, Optional, Union

from weil_wallet import Wallet, WeilClient
from weil_wallet.constants import SENTINEL_HOST
from weil_wallet.errors import WalletNotPermittedError
from weil_wallet.transaction import TransactionResult

from weil_ai.auth import (
    abuild_auth_headers,
    build_auth_headers,
    full_address_from_headers,
    verify_weil_request,
)
from weil_ai.mcp import (
    reset_current_wallet_addr,
    set_current_wallet_addr,
    wallet_has_purpose,
)

from weil_ai.tracer import (
    begin_trace,
    finish_trace,
    finish_trace_sync,
    is_async_stream,
    is_sync_stream,
    root_from_call,
    wrap_async_iterator,
    wrap_sync_iterator,
)

class WeilAgent:
    """Proxy wrapper that attaches a Weil wallet identity to any agent.

    Attributes not implemented by this proxy are forwarded to the wrapped
    agent. Weil-specific methods and properties (such as ``get_auth_headers``,
    ``audit``, and ``weil_wallet``) are handled by the proxy itself.
    """

    _WEIL_ATTRS = frozenset(
        {
            "_agent",
            "_wallet",
            "_weil_client",
            "_sentinel_host",
            "_verify",
            "_concurrency",
            "_credentials",
            "get_auth_headers",
            "aget_auth_headers",
            "audit",
            "audit_mcp_result",
            "weil_session",
            "stream_audit",
            "pensieve",
            "weil_wallet",
            "inner_agent",
            "remote",
            "setup_secure_a2a",
            "to_secure_a2a",
            "setup_secure_langchain",
            "setup_secure_strands",
            "_get_client",
            "_audit_async",
        }
    )

    def __init__(
        self,
        agent: Any = None,
        *,
        private_key_path: Optional[Union[str, Path]] = None,
        wallet: Optional[Wallet] = None,
        identity: Optional[str] = None,
        concurrency: Optional[int] = None,
        sentinel_host: Optional[str] = None,
        credentials: Optional[dict] = None,
        verify: bool = True,
        secure: bool = False,
        require_lock: bool = True,
        harden_process: bool = True,
        include_creds: bool = False,
    ) -> None:
        """Wrap *agent* with a Weil wallet.

        Exactly one of ``wallet``, ``private_key_path``, or ``identity`` must
        be provided. The wrapped object is otherwise left untouched: method
        calls and attribute access continue to flow to the original agent.

        Args:
            agent: Agent object to wrap.
            wallet: Pre-built :class:`Wallet`.
            private_key_path: Path to an account export JSON file.
            identity: Agent identity used to resolve the wallet from Sentinel.
            concurrency: Max concurrent in-flight Sentinel requests.
            sentinel_host: Override Sentinel node URL. Defaults to the
                ``SENTINEL_HOST`` environment variable, then the package
                default.
            verify: Whether to verify TLS certificates. Set ``False`` only for
                development hosts with self-signed certificates.
            credentials: Optional storage credentials used to resolve externally
                stored identity wallets.
            secure: Use native secure-memory wallet loading when available.
            require_lock: With ``secure=True``, fail if native key memory cannot
                be locked into RAM.
            harden_process: With ``secure=True``, disable core dumps before
                loading key material.
            include_creds: With ``secure=True`` and ``identity``, attach
                environment-provided AWS credentials to the native wallet fetch.
        """
        sources = sum(x is not None for x in (wallet, private_key_path, identity))
        if sources != 1:
            raise ValueError(
                "Provide exactly one of wallet=, private_key_path=, or identity=."
            )
        if secure and wallet is not None:
            raise ValueError("secure=True requires private_key_path= or identity=.")
        if secure and identity is not None and credentials is not None:
            warnings.warn(
                "credentials= is ignored with secure=True; the secure "
                "native backend reads AWS credentials from the environment. "
                "Attaching env-var credentials to the request.",
                stacklevel=2,
            )
            include_creds = True

        resolved_sentinel_host = (
            sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
        )
        client = None

        if wallet is None and identity is not None:
            if secure:
                wallet = Wallet.from_secure_identity(
                    identity,
                    sentinel_host=resolved_sentinel_host,
                    verify=verify,
                    require_lock=require_lock,
                    harden_process=harden_process,
                    include_creds=include_creds,
                )
            else:
                client = _run_sync(
                    WeilClient.from_identity(
                        identity,
                        concurrency=concurrency,
                        sentinel_host=resolved_sentinel_host,
                        credentials=credentials,
                        verify=verify,
                    )
                )
                wallet = client._wallet
        elif wallet is None:
            path = Path(private_key_path)
            if not path.is_file():
                raise FileNotFoundError(f"Account export file not found: {path}")
            wallet = Wallet.from_account_export_file(
                path,
                secure=secure,
                require_lock=require_lock,
                harden_process=harden_process,
            )
        object.__setattr__(self, "_agent", agent)
        object.__setattr__(self, "_wallet", wallet)
        object.__setattr__(self, "_weil_client", client)
        object.__setattr__(self, "_concurrency", concurrency)
        object.__setattr__(self, "_sentinel_host", resolved_sentinel_host)
        object.__setattr__(self, "_verify", verify)
        if not secure:
            object.__setattr__(self, "_credentials", credentials)
        # ADK holds the unwrapped agent in sub_agents; keep a way back to this
        # proxy so the agent's own wallet can still be found there.
        try:
            object.__setattr__(agent, "__weil_agent__", self)
        except Exception:
            pass

    # ------------------------------------------------------------------
    # Weil-specific public API
    # ------------------------------------------------------------------

    @property
    def weil_wallet(self) -> Wallet:
        """The :class:`Wallet` attached to this agent."""
        return object.__getattribute__(self, "_wallet")

    @property
    def inner_agent(self) -> Any:
        """The original agent object wrapped by this WeilAgent."""
        return object.__getattribute__(self, "_agent")

    def get_auth_headers(self, body: Any = None) -> dict:
        """Return signed auth headers for an MCP (or any HTTP) request.

        Pass the exact request body when the receiver verifies a body-bound
        signature; otherwise the signature covers the headers' timestamp and
        wallet address only.

        Ready to pass directly to an MCP transport::

            headers = agent.get_auth_headers()
            async with streamablehttp_client(url, headers=headers) as (...):
                ...

        Returns:
            Dict with keys ``X-Wallet-Address``, ``X-Signature``,
            ``X-Message``, and ``X-Timestamp``.
        """
        wallet = object.__getattribute__(self, "_wallet")
        client = self._get_client()
        signer = None
        if client._remote_signing:
            def signer(msg: bytes) -> str:
                return _run_sync(client.sign_message(json.loads(msg)))
        return build_auth_headers(wallet, signer=signer, body=body)

    async def aget_auth_headers(self, body: Any = None) -> dict:
        """Async :meth:`get_auth_headers`; use it from async code.

        Signs on the caller's own event loop. The sync version bridges through
        _run_sync for API-key wallets, which runs the Sentinel call on a new
        loop -- and this client's pooled connections belong to the first loop
        that used them.
        """
        wallet = object.__getattribute__(self, "_wallet")
        client = self._get_client()
        signer = None
        if client._remote_signing:

            async def signer(msg: bytes) -> str:
                return await client.sign_message(json.loads(msg))

        return await abuild_auth_headers(wallet, signer=signer, body=body)


    def remote(self, base_url: str) -> "WeilRemoteAgent":
        """Return a proxy for invoking a remote Weil agent over HTTP."""
        return WeilRemoteAgent(self, base_url)


    # Google ADK A2A (Agent-to-Agent) support
    def setup_secure_a2a(
        self,
        *,
        service: Optional[str] = None,
        audit: bool = True,
        verify_signature: bool = True,
        pensieve: bool = True,
    ) -> "WeilAgent":
        """Install Weil signing, verification, and audit hooks for Google ADK A2A.

        Args:
            service: Permission gate service name for inbound A2A calls.
            audit: Enable audit logging.
            verify_signature: Verify inbound caller signatures.
            pensieve: Install pensieve audit tools.

        Returns:
            Self for method chaining.
        """
        from weil_ai.adk import setup_secure_a2a

        setup_secure_a2a(
            self,
            service=service,
            audit=audit,
            verify_signature=verify_signature,
            pensieve=pensieve,
        )
        return self

    def to_secure_a2a(self, **kwargs: Any) -> Any:
        """Convert the wrapped ADK agent to A2A and add Weil verification.

        Args:
            **kwargs: Passed to ``weil_ai.adk.to_secure_a2a``.

        Returns:
            Secure A2A Starlette application.
        """
        from weil_ai.adk import to_secure_a2a

        return to_secure_a2a(self, **kwargs)

    # LangChain support
    def setup_secure_langchain(
        self,
        *,
        audit: bool = True,
        verify: bool = True,
        pensieve: bool = True,
    ) -> "WeilAgent":
        """Install Weil invocation context/audit hooks for a LangChain agent.

        Args:
            audit: Enable audit logging.
            verify: Verify TLS certificates for Sentinel connections.
            pensieve: Install pensieve audit tools.

        Returns:
            Self for method chaining.
        """
        from weil_ai.langchain import setup_secure_langchain

        setup_secure_langchain(self, audit=audit, verify=verify, pensieve=pensieve)
        return self

    def to_secure_http(self, *, service: Optional[str] = None) -> Any:
        """Serve this agent's methods over signed HTTP (``POST /invoke``).

        LangChain counterpart of ``to_secure_a2a``: callers are verified by
        signature and, with *service*, by on-chain permission. See
        :func:`agent_http_app`.
        """
        return agent_http_app(self, service=service)
    # Strands Agents support
    def setup_secure_strands(
        self,
        *,
        audit: bool = True,
        verify: bool = True,
        pensieve: bool = True,
    ) -> "WeilAgent":
        """Install Weil audit, trace and secured-tool hooks on a Strands agent.

        Also secures its sub-agents (agents used as tools), or the nodes of a
        wrapped Graph or Swarm, and signs remote A2A agents among them.

        Args:
            audit: Enable audit logging.
            verify: Verify TLS certificates for Sentinel connections.
            pensieve: Install pensieve memory tools.

        Returns:
            Self for method chaining.
        """
        from weil_ai.strands import setup_secure_strands

        setup_secure_strands(self, audit=audit, verify=verify, pensieve=pensieve)
        return self

    def audit(self, log: str) -> TransactionResult:
        """Write *log* to the on-chain auditor applet under this agent's identity.

        Safe to call from both sync and async contexts.
        """
        return _run_sync(self._audit_async(log))

    def stream_audit(self, topic: str = "audits") -> Callable[[Callable], Callable]:
        """Return a decorator that publishes wrapped calls to a stream topic."""
        return self._get_client().stream_audit(topic)

    async def audit_mcp_result(self, result: Any) -> Any:
        """Submit a returned MCP audit payload with this agent's wallet."""
        from weil_ai.mcp import AUDIT_FIELD

        entry = (getattr(result, "meta", None) or {}).get(AUDIT_FIELD)
        if entry:
            await self._audit_async(entry)
        return result

    def weil_session(
        self,
        session: Any,
        *,
        raise_audit_errors: bool = False,
    ) -> "WeilMcpSession":
        """Wrap an MCP client session and audit tool results automatically."""
        return WeilMcpSession(
            self,
            session,
            raise_audit_errors=raise_audit_errors,
        )

    def pensieve(self, **kwargs: Any) -> Any:
        """Return a Pensieve graph-memory client for this agent."""
        from weil_wallet.pensieve import Pensieve

        return Pensieve(self, **kwargs)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get_client(self) -> WeilClient:
        client = object.__getattribute__(self, "_weil_client")
        if client is None:
            wallet = object.__getattribute__(self, "_wallet")
            sentinel_host = object.__getattribute__(self, "_sentinel_host")
            client = WeilClient(
                wallet,
                object.__getattribute__(self, "_concurrency"),
                sentinel_host=sentinel_host,
                verify=object.__getattribute__(self, "_verify"),
            )
            # A masked wallet without a local secret key delegates signing to Sentinel.
            if wallet._source_json is not None and wallet.secret_key is None:
                client._remote_signing = True
                client._credentials = object.__getattribute__(self, "_credentials")

            object.__setattr__(self, "_weil_client", client)
        return client

    def _wallet_auth_address(self) -> str:
        wallet = object.__getattribute__(self, "_wallet")
        return wallet.get_address()

    async def _audit_async(self, log: str) -> TransactionResult:
        return await self._get_client().audit(log)

    # ------------------------------------------------------------------
    # Proxy: forward everything else to the wrapped agent
    # ------------------------------------------------------------------

    def __getattr__(self, name: str) -> Any:
        attr = getattr(object.__getattribute__(self, "_agent"), name)
        if name.startswith("_") or not callable(attr):
            return attr
        return self._trace_callable(attr)

    def _trace_callable(self, attr: Callable) -> Callable:
        """Wrap a forwarded agent method with trace lifecycle handling.

        The wrapped method starts a trace from the call arguments, lets the
        underlying agent method run unchanged, and finishes the trace after a
        sync result, awaitable result, or stream has completed.

        An async method starts its trace when its coroutine starts running,
        not when it is created: ``asyncio.run(agent.ainvoke(...))`` runs the
        coroutine in a copy of the caller's context, and a trace begun outside
        that copy could not be cleared inside it, leaving every later run of
        the caller untraced.
        """
        if inspect.iscoroutinefunction(attr):

            @functools.wraps(attr)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                trace = begin_trace(self, root_from_call(args, kwargs))
                try:
                    result = attr(*args, **kwargs)
                except Exception:
                    try:
                        await finish_trace(trace)
                    except Exception:
                        pass
                    raise
                return await self._finish_awaitable_trace(result, trace)

            return async_wrapper

        @functools.wraps(attr)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            trace = begin_trace(self, root_from_call(args, kwargs))
            try:
                result = attr(*args, **kwargs)
                if inspect.isawaitable(result):
                    return self._finish_awaitable_trace(result, trace)
                if is_sync_stream(result):
                    return wrap_sync_iterator(result, trace)
                try:
                    finish_trace_sync(trace)
                except Exception:
                    pass
                return result
            except Exception:
                try:
                    finish_trace_sync(trace)
                except Exception:
                    pass
                raise

        return wrapper

    async def _finish_awaitable_trace(self, result: Any, trace: Any) -> Any:
        """Await a forwarded async call and finish its trace.

        If the awaited value is an async stream, trace completion is deferred
        until the stream wrapper is exhausted. Exceptions still close the trace
        before being re-raised.
        """
        try:
            value = await result
            if is_async_stream(value):
                return wrap_async_iterator(value, trace)
            try:
                await finish_trace(trace)
            except Exception:
                pass
            return value
        except Exception:
            try:
                await finish_trace(trace)
            except Exception:
                pass
            raise

    def __setattr__(self, name: str, value: Any) -> None:
        if name in WeilAgent._WEIL_ATTRS:
            object.__setattr__(self, name, value)
        else:
            setattr(object.__getattribute__(self, "_agent"), name, value)

    async def close(self) -> None:
        """Close the underlying WeilClient if one was created."""
        client = object.__getattribute__(self, "_weil_client")
        if client is not None:
            await client.close()
            object.__setattr__(self, "_weil_client", None)

    async def __aenter__(self) -> "WeilAgent":
        return self

    async def __aexit__(self, *args: Any) -> None:
        await self.close()


class WeilMcpSession:
    """MCP client-session proxy that submits Weil audit metadata automatically."""

    def __init__(
        self,
        agent: WeilAgent,
        session: Any,
        *,
        raise_audit_errors: bool = False,
    ) -> None:
        self._agent = agent
        self._session = session
        self._raise_audit_errors = raise_audit_errors
        self.last_audit_error: Optional[BaseException] = None

    def __getattr__(self, name: str) -> Any:
        return getattr(self._session, name)

    async def initialize(self, *args: Any, **kwargs: Any) -> Any:
        """Initialize the underlying MCP session."""
        return await self._session.initialize(*args, **kwargs)

    async def call_tool(self, *args: Any, **kwargs: Any) -> Any:
        """Call a tool and submit any audit payload attached to its result."""
        result = await self._session.call_tool(*args, **kwargs)
        self.last_audit_error = None
        try:
            await self._agent.audit_mcp_result(result)
        except Exception as error:
            self.last_audit_error = error
            if self._raise_audit_errors:
                raise
        return result


class WeilRemoteAgent:
    """Client-side proxy for a remote Weil agent HTTP app."""

    def __init__(self, caller: WeilAgent, base_url: str) -> None:
        self._caller = caller
        self._base_url = base_url.rstrip("/")

    def __getattr__(self, name: str) -> Callable:
        def remote_call(*args: Any, **kwargs: Any) -> Any:
            payload = {"method": name, "args": list(args), "kwargs": kwargs}
            resp = httpx.post(
                f"{self._base_url}/invoke",
                headers=self._caller.get_auth_headers(body=payload),
                json=payload,
                timeout=60.0,
            )
            resp.raise_for_status()
            return resp.json().get("result")

        return remote_call

    async def acall(self, name: str, *args: Any, **kwargs: Any) -> Any:
        """Call a named method on the remote agent asynchronously."""
        payload = {"method": name, "args": list(args), "kwargs": kwargs}
        async with httpx.AsyncClient(base_url=self._base_url, timeout=60.0) as client:
            resp = await client.post(
                "/invoke",
                headers=await self._caller.aget_auth_headers(body=payload),
                json=payload,
            )
        resp.raise_for_status()
        return resp.json().get("result")


def agent_http_app(agent: WeilAgent, *, service: Optional[str] = None) -> Any:
    """Create a small Starlette app exposing public methods on *agent*.

    Requests must include the body-bound Weil headers produced by
    ``WeilAgent.get_auth_headers(body=payload)``. When *service* is set, the
    verified caller must also hold the Execution or Management purpose on that service
    (the same gate ``setup_secure_a2a(service=...)`` applies to ADK agents);
    otherwise the request is rejected with 403 before any agent code runs.
    The request body is:

        {"method": "run", "args": [...], "kwargs": {...}}
    """
    try:
        from starlette.applications import Starlette
        from starlette.requests import Request
        from starlette.responses import JSONResponse
        from starlette.routing import Route
    except ImportError as exc:
        raise ImportError(
            "starlette is required for agent_http_app(); install it with starlette"
        ) from exc

    async def invoke(request: Request) -> JSONResponse:
        try:
            body = await request.json()
        except ValueError:
            return JSONResponse({"error": "Invalid JSON body"}, status_code=400)
        if not isinstance(body, dict):
            return JSONResponse({"error": "Invalid request body"}, status_code=400)
        if not verify_weil_request(request.headers, body):
            return JSONResponse(
                {"error": "Wallet address verification failed"}, status_code=401
            )

        # This header carries the public-key-derived address; the signed
        # canonical address (including the pod counter) is recovered below.
        wallet_address = request.headers.get("X-Wallet-Address")
        method = body.get("method")
        if not isinstance(method, str) or method.startswith("_"):
            return JSONResponse({"error": "Invalid method"}, status_code=400)

        inner = object.__getattribute__(agent, "_agent")
        if not hasattr(inner, method):
            return JSONResponse({"error": "Method not found"}, status_code=404)

        args = body.get("args", [])
        kwargs = body.get("kwargs", {})
        # Identity lookups key on the canonical address (pod counter + key
        # hash), which the client signs. Older clients omit it; fall back to
        # the key-derived form.
        caller_addr = (
            full_address_from_headers(request.headers) or wallet_address
        )
        sentinel_host = object.__getattribute__(agent, "_sentinel_host") or SENTINEL_HOST
        verify = object.__getattribute__(agent, "_verify")
        if service:
            try:
                permitted = await wallet_has_purpose(
                    caller_addr,
                    service,
                    purposes=("Execution", "Management"),
                    sentinel_host=sentinel_host,
                    verify=verify,
                )
            except Exception as exc:
                return JSONResponse(
                    {"error": f"permission check for service {service!r} failed: {exc}"},
                    status_code=403,
                )
            if not permitted:
                exc = WalletNotPermittedError(caller_addr, service)
                return JSONResponse({"error": str(exc)}, status_code=403)
        tokens = set_current_wallet_addr(
            caller_addr, sentinel_host=sentinel_host, verify=verify
        )
        try:
            target = getattr(agent, method)
            result = target(*args, **kwargs)
            if inspect.isawaitable(result):
                result = await result
            return JSONResponse({"result": result})
        except WalletNotPermittedError as exc:
            return JSONResponse({"error": str(exc)}, status_code=403)
        finally:
            reset_current_wallet_addr(tokens)

    return Starlette(routes=[Route("/invoke", invoke, methods=["POST"])])


def weil_agent(
    key_or_wallet: Optional[Union[str, Path, Wallet]] = None,
    *,
    identity: Optional[str] = None,
    concurrency: Optional[int] = None,
    sentinel_host: Optional[str] = None,
    credentials: Optional[dict] = None,
    verify: bool = True,
    secure: bool = False,
    require_lock: bool = True,
    harden_process: bool = True,
    include_creds: bool = False,
) -> Any:
    """Decorator factory that binds a Weil wallet to an agent factory function.

    Pass either a path to an account export JSON file or a pre-built
    :class:`~weil_wallet.Wallet`. The wallet is resolved once at decoration
    time and shared across all calls to the factory.

    Example::

        import weil_ai

        @weil_ai.agent("account.json")
        def create_agent():
            return MyAgent()

        # or with a Wallet object:
        @weil_ai.agent(wallet)
        def create_agent():
            return MyAgent()

        agent = create_agent()
        agent.get_auth_headers()    # signed MCP headers
        agent.run("What is 2+2?")  # original method unchanged
        agent.audit("ran query")   # on-chain log

    Args:
        key_or_wallet: Path to an account export JSON file, or a pre-built
            :class:`~weil_wallet.Wallet`.
        identity: Agent identity used to resolve a wallet from Sentinel.
        concurrency: Max concurrent Sentinel requests for the wrapped agent.
        sentinel_host: Override the Sentinel node URL.
        credentials: Optional storage credentials for externally stored
            identity wallets.
        verify: Whether to verify TLS certificates.
        secure: Use native secure-memory wallet loading when available.
        require_lock: With ``secure=True``, fail if native key memory cannot be
            locked into RAM.
        harden_process: With ``secure=True``, disable core dumps before loading
            key material.
        include_creds: With ``secure=True`` and ``identity``, attach
            environment-provided AWS credentials to the native wallet fetch.
    """
    if (key_or_wallet is None) == (identity is None):
        raise ValueError("Provide exactly one of key_or_wallet or identity=.")
    if secure and isinstance(key_or_wallet, Wallet):
        raise ValueError("secure=True requires a wallet file path or identity=.")
    if secure and identity is not None and credentials is not None:
        warnings.warn(
            "credentials= is ignored with secure=True; the secure "
            "native backend reads AWS credentials from the environment. "
            "Attaching env-var credentials to the request.",
            stacklevel=2,
        )
        include_creds = True

    resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST")

    if identity is not None:
        resolved_sentinel_host = (
            sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
        )
        if secure:
            wallet = Wallet.from_secure_identity(
                identity,
                sentinel_host=resolved_sentinel_host,
                verify=verify,
                require_lock=require_lock,
                harden_process=harden_process,
                include_creds=include_creds,
            )
        else:
            wallet = _run_sync(
                WeilClient.get_agent_wallet(
                    identity,
                    sentinel_host=resolved_sentinel_host,
                    verify=verify,
                    credentials=credentials,
                )
            )
    elif isinstance(key_or_wallet, Wallet):
        wallet = key_or_wallet
    else:
        path = Path(key_or_wallet)
        if not path.is_file():
            raise FileNotFoundError(f"Account export file not found: {path}")
        wallet = Wallet.from_account_export_file(
            path,
            secure=secure,
            require_lock=require_lock,
            harden_process=harden_process,
        )

    def decorator(fn: Any) -> Any:
        def wrapper(*args: Any, **kwargs: Any) -> WeilAgent:
            factory_agent = WeilAgent(
                None,
                wallet=wallet,
                concurrency=concurrency,
                sentinel_host=resolved_sentinel_host,
                verify=verify,
                credentials=credentials,
            )
            try:
                from weil_ai.langchain import _factory_identity
            except Exception:
                inner = fn(*args, **kwargs)
            else:
                with _factory_identity(factory_agent):
                    inner = fn(*args, **kwargs)
            return WeilAgent(
                inner,
                wallet=wallet,
                concurrency=concurrency,
                sentinel_host=resolved_sentinel_host,
                verify=verify,
                credentials=credentials,
            )

        wrapper.__name__ = getattr(fn, "__name__", "create_agent")
        wrapper.__doc__ = fn.__doc__
        return wrapper

    return decorator


def _run_sync(coro: Any) -> Any:
    from weil_ai.mcp import _run_sync as run_sync

    return run_sync(coro)
