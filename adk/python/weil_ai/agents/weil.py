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

import contextvars
import functools
import inspect
import json
import os
import warnings
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
    verify_weil_signature,
)
from weil_ai.mcp import (
    wallet_has_execution_permission,
    wallet_has_management_permission,
)


_CALLER_WALLET_ADDR: contextvars.ContextVar[str] = contextvars.ContextVar(
    "weil_agent_caller_wallet_addr",
    default="",
)

_VERIFIABLE_ATTR = "__weil_verifiable_svc_name__"


def verifiable(svc_name: str) -> Callable:
    """Mark an agent method as requiring Execution permission on *svc_name*.

    The check is enforced by :class:`WeilAgent` when the method is called through
    the wrapped agent returned by ``@weil_ai.agent(...)``.
    """

    def decorator(func: Callable) -> Callable:
        setattr(func, _VERIFIABLE_ATTR, svc_name)
        return func

    return decorator


def _verifiable_service(attr: Any) -> Optional[str]:
    return getattr(attr, _VERIFIABLE_ATTR, None) or getattr(
        getattr(attr, "__func__", None),
        _VERIFIABLE_ATTR,
        None,
    )


class WeilAgent:
    """Proxy wrapper that attaches a Weil wallet identity to any agent.

    All attribute access is forwarded to the wrapped agent except for the
    Weil-specific names: ``get_auth_headers``, ``audit``, ``weil_wallet``.
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
            "pensieve",
            "weil_wallet",
            "inner_agent",
            "as_caller",
            "call",
            "acall",
            "remote",
            "setup_secure_a2a",
            "secure_a2a_app",
            "to_secure_a2a",
            "setup_secure_langchain",
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

        Exactly one of *wallet*, *private_key_path*, or *identity* must be provided.

        Args:
            agent:            Any agent object to wrap.
            wallet:           Pre-built :class:`Wallet` (takes precedence over
                              *private_key_path*).
            private_key_path: Path to an account export JSON file (produced by
                              the CLI's ``wallet export-account`` command).
            identity:          Agent identity used to resolve the wallet.
            concurrency:      Max concurrent in-flight requests for the client.
            sentinel_host:    Override Sentinel node URL (defaults to
                              ``SENTINEL_HOST`` env var or the production endpoint).
            verify:           Whether to verify TLS certificates (set False for
                              self-signed / development Sentinel nodes).
            credentials:      Optional storage credentials (e.g. AWS S3 keys) used
                              to resolve an externally stored wallet from the
                              agent identity.
            include_creds:    With ``secure=True`` + ``identity``, attach
                              env-var AWS credentials to the wallet fetch. Set
                              True only for external wallets stored under those
                              credentials.
        """
        sources = sum(x is not None for x in (wallet, private_key_path, identity))
        if sources != 1:
            raise ValueError("Provide exactly one of wallet=, private_key_path=, or identity=.")
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

        resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
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

    @contextmanager
    def as_caller(self) -> Iterator[None]:
        """Use this agent's wallet as the caller for local verifiable calls."""
        token = _CALLER_WALLET_ADDR.set(self._wallet_auth_address())
        try:
            yield
        finally:
            _CALLER_WALLET_ADDR.reset(token)

    def call(self, target: Callable, *args: Any, **kwargs: Any) -> Any:
        """Call another wrapped agent method as this agent."""
        with self.as_caller():
            return target(*args, **kwargs)

    async def acall(self, target: Callable, *args: Any, **kwargs: Any) -> Any:
        """Async variant of :meth:`call`."""
        with self.as_caller():
            result = target(*args, **kwargs)
            if inspect.isawaitable(result):
                return await result
            return result

    def remote(self, base_url: str) -> "WeilRemoteAgent":
        """Return a proxy for invoking a remote Weil agent over HTTP."""
        return WeilRemoteAgent(self, base_url)

    def setup_secure_a2a(
        self,
        *,
        service: Optional[str] = None,
        audit: bool = True,
        verify: bool = True,
        pensieve: bool = True,
    ) -> "WeilAgent":
        """Install Weil signing, verification, and audit hooks for Google ADK A2A."""
        from weil_ai.adk import setup_secure_a2a

        setup_secure_a2a(
            self,
            service=service,
            audit=audit,
            verify=verify,
            pensieve=pensieve,
        )
        return self

    def secure_a2a_app(
        self,
        app: Any,
        *,
        service: Optional[str] = None,
        audit: bool = True,
        verify: bool = True,
    ) -> Any:
        """Wrap an ADK A2A Starlette app with Weil request verification."""
        from weil_ai.adk import secure_a2a_app

        return secure_a2a_app(
            app,
            self,
            service=service,
            audit=audit,
            verify=verify,
        )

    def to_secure_a2a(self, **kwargs: Any) -> Any:
        """Convert the wrapped ADK agent to A2A and add Weil verification."""
        from weil_ai.adk import to_secure_a2a

        return to_secure_a2a(self, **kwargs)

    def setup_secure_langchain(
        self,
        *,
        audit: bool = True,
        verify: bool = True,
        pensieve: bool = True,
    ) -> "WeilAgent":
        """Install Weil invocation context/audit hooks for a LangChain agent."""
        from weil_ai.langchain import setup_secure_langchain

        setup_secure_langchain(self, audit=audit, verify=verify, pensieve=pensieve)
        return self

    def audit(self, log: str) -> TransactionResult:
        """Write *log* to the on-chain auditor applet under this agent's identity.

        Safe to call from both sync and async contexts.
        """
        return _run_sync(self._audit_async(log))

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

    async def _verify_caller_permission(self, caller_addr: str, svc_name: str) -> None:
        """Allow callers holding either Management or Execution purpose.

        Mirrors weil_ai.secured_user: an admin key (Management) passes without
        also needing Execution.
        """
        sentinel_host = object.__getattribute__(self, "_sentinel_host")
        verify = object.__getattribute__(self, "_verify")
        if await wallet_has_management_permission(
            caller_addr,
            svc_name,
            sentinel_host=sentinel_host,
            verify=verify,
        ):
            return
        if await wallet_has_execution_permission(
            caller_addr,
            svc_name,
            sentinel_host=sentinel_host,
            verify=verify,
        ):
            return
        raise WalletNotPermittedError(caller_addr, svc_name)

    async def _audit_async(self, log: str) -> TransactionResult:
        return await self._get_client().audit(log)

    # ------------------------------------------------------------------
    # Proxy: forward everything else to the wrapped agent
    # ------------------------------------------------------------------

    def __getattr__(self, name: str) -> Any:
        return getattr(object.__getattribute__(self, "_agent"), name)

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


class WeilRemoteAgent:
    """Client-side proxy for a remote Weil agent HTTP app."""

    def __init__(self, caller: WeilAgent, base_url: str) -> None:
        self._caller = caller
        self._base_url = base_url.rstrip("/")

    def __getattr__(self, name: str) -> Callable:
        def remote_call(*args: Any, **kwargs: Any) -> Any:
            import httpx

            resp = httpx.post(
                f"{self._base_url}/invoke",
                headers=self._caller.get_auth_headers(),
                json={"method": name, "args": args, "kwargs": kwargs},
                timeout=60.0,
            )
            resp.raise_for_status()
            return resp.json().get("result")

        return remote_call

    async def acall(self, name: str, *args: Any, **kwargs: Any) -> Any:
        import httpx

        async with httpx.AsyncClient(base_url=self._base_url, timeout=60.0) as client:
            resp = await client.post(
                "/invoke",
                headers=self._caller.get_auth_headers(),
                json={"method": name, "args": args, "kwargs": kwargs},
            )
        resp.raise_for_status()
        return resp.json().get("result")


def agent_http_app(agent: WeilAgent) -> Any:
    """Create a small Starlette app exposing public methods on *agent*.

    Requests must include the same signed Weil headers produced by
    ``WeilAgent.get_auth_headers()``. The request body is:

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
        wallet_address = request.headers.get("X-Wallet-Address")
        signature_hex = request.headers.get("X-Signature")
        message = request.headers.get("X-Message")
        timestamp = request.headers.get("X-Timestamp")

        if not verify_weil_signature(wallet_address, signature_hex, message, timestamp):
            return JSONResponse({"error": "Wallet address verification failed"}, status_code=401)

        body = await request.json()
        method = body.get("method")
        if not isinstance(method, str) or method.startswith("_"):
            return JSONResponse({"error": "Invalid method"}, status_code=400)

        inner = object.__getattribute__(agent, "_agent")
        if not hasattr(inner, method):
            return JSONResponse({"error": "Method not found"}, status_code=404)

        args = body.get("args", [])
        kwargs = body.get("kwargs", {})
        token = _CALLER_WALLET_ADDR.set(wallet_address)
        try:
            target = getattr(agent, method)
            result = target(*args, **kwargs)
            if inspect.isawaitable(result):
                result = await result
            return JSONResponse({"result": result})
        except WalletNotPermittedError as exc:
            return JSONResponse({"error": str(exc)}, status_code=403)
        finally:
            _CALLER_WALLET_ADDR.reset(token)

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
        key_or_wallet: Path to an account export JSON file (``str`` or
                       :class:`~pathlib.Path`), or a pre-built
                       :class:`~weil_wallet.Wallet`.
        sentinel_host: Override the Sentinel node URL.
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
        resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
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
