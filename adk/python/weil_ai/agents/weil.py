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

import asyncio
import concurrent.futures
import json
import os
import warnings
from pathlib import Path
from typing import Any, Optional, Union

from weil_wallet import Wallet, WeilClient
from weil_wallet.constants import SENTINEL_HOST
from weil_wallet.transaction import TransactionResult

from weil_ai.auth import build_auth_headers


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
            "audit",
            "weil_wallet",
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
        api_key: Optional[str] = None,
        concurrency: Optional[int] = None,
        sentinel_host: Optional[str] = None,
        credentials: Optional[dict] = None,
        verify: bool = True,
        secure_wallet: bool = False,
        require_lock: bool = True,
        harden_process: bool = True,
        include_creds: bool = False,
    ) -> None:
        """Wrap *agent* with a Weil wallet.

        Exactly one of *wallet*, *private_key_path*, or *api_key* must be provided.

        Args:
            agent:            Any agent object to wrap.
            wallet:           Pre-built :class:`Wallet` (takes precedence over
                              *private_key_path*).
            private_key_path: Path to an account export JSON file (produced by
                              the CLI's ``wallet export-account`` command).
            api_key:          Agent Registry API key used to resolve the wallet.
            concurrency:      Max concurrent in-flight requests for the client.
            sentinel_host:    Override Sentinel node URL (defaults to
                              ``SENTINEL_HOST`` env var or the production endpoint).
            verify:           Whether to verify TLS certificates (set False for
                              self-signed / development Sentinel nodes).
            credentials:      Optional storage credentials (e.g. AWS S3 keys) used
                              to resolve an externally stored wallet from the
                              Agent Registry API key.
            include_creds:    With ``secure_wallet=True`` + ``api_key``, attach
                              env-var AWS credentials to the wallet fetch. Set
                              True only for external wallets stored under those
                              creds.
        """
        sources = sum(x is not None for x in (wallet, private_key_path, api_key))
        if sources != 1:
            raise ValueError("Provide exactly one of wallet=, private_key_path=, or api_key=.")
        if secure_wallet and wallet is not None:
            raise ValueError("secure_wallet=True requires private_key_path= or api_key=.")
        if secure_wallet and api_key is not None and credentials is not None:
            warnings.warn(
                "credentials= is ignored with secure_wallet=True; the secure "
                "native backend reads AWS credentials from the environment. "
                "Attaching env-var creds to the request.",
                stacklevel=2,
            )
            include_creds = True

        resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
        client = None

        if wallet is None and api_key is not None:
            if secure_wallet:
                wallet = Wallet.from_secure_api_key(
                    api_key,
                    sentinel_host=resolved_sentinel_host,
                    verify=verify,
                    require_lock=require_lock,
                    harden_process=harden_process,
                    include_creds=include_creds,
                )
            else:
                client = _run_sync(
                     WeilClient.from_api_key(
                        api_key,
                        concurrency=concurrency,
                        sentinel_host=resolved_sentinel_host,
                        creds=credentials,
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
                secure=secure_wallet,
                require_lock=require_lock,
                harden_process=harden_process,
            )
        object.__setattr__(self, "_agent", agent)
        object.__setattr__(self, "_wallet", wallet)
        object.__setattr__(self, "_weil_client", client)
        object.__setattr__(self, "_concurrency", concurrency)
        object.__setattr__(self, "_sentinel_host", resolved_sentinel_host)
        object.__setattr__(self, "_verify", verify)
        if not secure_wallet:
            object.__setattr__(self, "_credentials", credentials)

    # ------------------------------------------------------------------
    # Weil-specific public API
    # ------------------------------------------------------------------

    @property
    def weil_wallet(self) -> Wallet:
        """The :class:`Wallet` attached to this agent."""
        return object.__getattribute__(self, "_wallet")

    def get_auth_headers(self) -> dict:
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
        return build_auth_headers(wallet, signer=signer)

    def audit(self, log: str) -> TransactionResult:
        """Write *log* to the on-chain auditor applet under this agent's identity.

        Safe to call from both sync and async contexts.
        """
        return _run_sync(self._audit_async(log))

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


def weil_agent(
    key_or_wallet: Optional[Union[str, Path, Wallet]] = None,
    *,
    api_key: Optional[str] = None,
    concurrency: Optional[int] = None,
    sentinel_host: Optional[str] = None,
    credentials: Optional[dict] = None,
    verify: bool = True,
    secure_wallet: bool = False,
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
    if (key_or_wallet is None) == (api_key is None):
        raise ValueError("Provide exactly one of key_or_wallet or api_key=.")
    if secure_wallet and isinstance(key_or_wallet, Wallet):
        raise ValueError("secure_wallet=True requires a wallet file path or api_key=.")
    if secure_wallet and api_key is not None and credentials is not None:
        warnings.warn(
            "credentials= is ignored with secure_wallet=True; the secure "
            "native backend reads AWS credentials from the environment. "
            "Attaching env-var creds to the request.",
            stacklevel=2,
        )
        include_creds = True

    resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST")

    if api_key is not None:
        resolved_sentinel_host = sentinel_host or os.environ.get("SENTINEL_HOST") or SENTINEL_HOST
        if secure_wallet:
            wallet = Wallet.from_secure_api_key(
                api_key,
                sentinel_host=resolved_sentinel_host,
                verify=verify,
                require_lock=require_lock,
                harden_process=harden_process,
                include_creds=include_creds,
            )
        else:
            wallet = _run_sync(
                WeilClient.get_agent_wallet(
                    api_key,
                    sentinel_host=resolved_sentinel_host,
                    verify=verify,
                    creds=credentials,
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
            secure=secure_wallet,
            require_lock=require_lock,
            harden_process=harden_process,
        )

    def decorator(fn: Any) -> Any:
        def wrapper(*args: Any, **kwargs: Any) -> WeilAgent:
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
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        return asyncio.run(coro)

    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        return pool.submit(asyncio.run, coro).result()
