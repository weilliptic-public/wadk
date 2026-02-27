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
import os
from pathlib import Path
from typing import Any, Optional, Union

from weil_wallet import PrivateKey, Wallet, WeilClient
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
            "get_auth_headers",
            "audit",
            "weil_wallet",
            "_get_client",
            "_audit_async",
        }
    )

    def __init__(
        self,
        agent: Any,
        *,
        private_key_path: Optional[Union[str, Path]] = None,
        wallet: Optional[Wallet] = None,
        sentinel_host: Optional[str] = None,
    ) -> None:
        """Wrap *agent* with a Weil wallet.

        Exactly one of *wallet* or *private_key_path* must be provided.

        Args:
            agent:            Any agent object to wrap.
            wallet:           Pre-built :class:`Wallet` (takes precedence over
                              *private_key_path*).
            private_key_path: Path to a ``.wc`` hex-encoded private key file.
            sentinel_host:    Override Sentinel node URL (defaults to
                              ``SENTINEL_HOST`` env var or the production endpoint).
        """
        if wallet is None and private_key_path is None:
            raise ValueError("Provide either wallet= or private_key_path=.")

        if wallet is None:
            path = Path(private_key_path)
            if not path.is_file():
                raise FileNotFoundError(f"Private key file not found: {path}")
            wallet = Wallet(PrivateKey.from_file(path))

        object.__setattr__(self, "_agent", agent)
        object.__setattr__(self, "_wallet", wallet)
        object.__setattr__(self, "_weil_client", None)
        object.__setattr__(
            self, "_sentinel_host", sentinel_host or os.environ.get("SENTINEL_HOST")
        )

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
        return build_auth_headers(object.__getattribute__(self, "_wallet"))

    def audit(self, log: str) -> TransactionResult:
        """Write *log* to the on-chain auditor applet under this agent's identity.

        Safe to call from both sync and async contexts.
        """
        try:
            asyncio.get_running_loop()
            with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                return pool.submit(asyncio.run, self._audit_async(log)).result()
        except RuntimeError:
            return asyncio.run(self._audit_async(log))

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get_client(self) -> WeilClient:
        client = object.__getattribute__(self, "_weil_client")
        if client is None:
            sentinel_host = object.__getattribute__(self, "_sentinel_host")
            client = WeilClient(
                object.__getattribute__(self, "_wallet"),
                sentinel_host=sentinel_host,
            )
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
    key_or_wallet: Union[str, Path, Wallet],
    *,
    sentinel_host: Optional[str] = None,
) -> Any:
    """Decorator factory that binds a Weil wallet to an agent factory function.

    Pass either a path to a ``.wc`` private key file or a pre-built
    :class:`~weil_wallet.Wallet`. The wallet is resolved once at decoration
    time and shared across all calls to the factory.

    Example::

        import weil_ai

        @weil_ai.agent("private_key.wc")
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
        key_or_wallet: Path to a ``.wc`` private key file (``str`` or
                       :class:`~pathlib.Path`), or a pre-built
                       :class:`~weil_wallet.Wallet`.
        sentinel_host: Override the Sentinel node URL.
    """
    if isinstance(key_or_wallet, Wallet):
        wallet = key_or_wallet
    else:
        path = Path(key_or_wallet)
        if not path.is_file():
            raise FileNotFoundError(f"Private key file not found: {path}")
        wallet = Wallet(PrivateKey.from_file(path))

    def decorator(fn: Any) -> Any:
        def wrapper(*args: Any, **kwargs: Any) -> WeilAgent:
            inner = fn(*args, **kwargs)
            return WeilAgent(inner, wallet=wallet, sentinel_host=sentinel_host)

        wrapper.__name__ = getattr(fn, "__name__", "create_agent")
        wrapper.__doc__ = fn.__doc__
        return wrapper

    return decorator
