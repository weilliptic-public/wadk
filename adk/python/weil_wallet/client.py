"""WeilClient and WeilContractClient for executing applet methods on WeilChain."""

from __future__ import annotations
import asyncio
import functools
import inspect
import json
from typing import Any, AsyncIterator, Callable, Optional
import httpx
from .api.platform_api import PlatformApi
from .api.request import SubmitTxnRequest, Transaction, UserTransaction, Verifier
from .constants import DEFAULT_CONCURRENCY, SENTINEL_HOST
from .contract import ContractId
from .streaming import ByteStream
from .transaction import BaseTransaction, TransactionHeader, TransactionResult
from .utils import current_time_millis, get_address_from_public_key
from .wallet import Wallet

AUDIT_APPLET_SVC_NAME = "auditor"


class WeilClient:
    """High-level client for WeilChain applet methods.

    Holds an HTTP client, a signer Wallet, and a concurrency limiter.
    """

    def __init__(
        self,
        wallet: Wallet,
        concurrency: Optional[int] = None,
        *,
        sentinel_host: Optional[str] = "https://sentinel.unweil.me",
        verify: bool = True,
    ) -> None:
        """Create a WeilClient.

        Args:
            wallet: Signing wallet (holds the private key).
            concurrency: Max concurrent in-flight requests. Defaults to DEFAULT_CONCURRENCY.
            sentinel_host: Base URL of the Sentinel node. Defaults to the production endpoint.
            verify: Whether to verify TLS certificates (set False for self-signed certs).
        """
        self._wallet = wallet
        self._concurrency = (
            concurrency if concurrency is not None else DEFAULT_CONCURRENCY
        )
        self._semaphore = asyncio.Semaphore(self._concurrency)
        self._sentinel_host = sentinel_host or SENTINEL_HOST
        self._http_client = httpx.AsyncClient(
            base_url=self._sentinel_host.rstrip("/"),
            verify=verify,
            timeout=60.0,
        )
        self._audit_contract_id: Optional[ContractId] = None
        self._audit_contract_id_lock = asyncio.Lock()

    def to_contract_client(self, contract_id: ContractId) -> "WeilContractClient":
        """Create a WeilContractClient bound to a specific ContractId."""
        return WeilContractClient(contract_id=contract_id, client=self)

    @staticmethod
    async def get_applet_id_for_name(
        http_client: httpx.AsyncClient, name: str
    ) -> ContractId:
        """Resolve a service name to its ContractId via the Sentinel API.

        Args:
            name: The registered applet service name (e.g. ``"auditor"``).
            verify: Whether to verify TLS certificates. Pass False only for
                self-signed / development Sentinel nodes.

        Returns:
            The ContractId for the named applet.

        Raises:
            RuntimeError: If the Sentinel API returns an error response.
        """
        resp = await http_client.post(
            "/get_applet_address",
            json={"svc_name": name},
        )
        resp.raise_for_status()
        data = resp.json()
        if "Ok" in data:
            return ContractId(data["Ok"])
        raise RuntimeError(f"get_applet_address failed: {data.get('Err', data)}")

    def wallet_addr(self) -> str:
        """Return the hex-encoded wallet address derived from the signing public key."""
        public_key = self._wallet.get_public_key()
        from_addr = get_address_from_public_key(public_key)

        return from_addr

    async def execute(
        self,
        contract_id: ContractId,
        method_name: str,
        method_args: str,
        should_hide_args: bool = True,
        is_non_blocking: bool = False,
    ) -> TransactionResult:
        """Execute a contract method and return the transaction result."""
        return await self.to_contract_client(contract_id).execute(
            method_name, method_args, should_hide_args, is_non_blocking
        )

    async def execute_with_streaming(
        self,
        contract_id: ContractId,
        method_name: str,
        method_args: str,
    ) -> ByteStream:
        """Execute a contract method and return a streaming response."""
        return await self.to_contract_client(contract_id).execute_with_streaming(
            method_name, method_args
        )

    async def _get_audit_contract_id(self) -> ContractId:
        """Resolve and cache the audit applet contract address from the Sentinel API."""
        async with self._audit_contract_id_lock:
            if self._audit_contract_id is not None:
                return self._audit_contract_id
            resp = await self._http_client.post(
                "/get_applet_address",
                json={"svc_name": AUDIT_APPLET_SVC_NAME},
            )
            resp.raise_for_status()
            data = resp.json()
            if "Ok" in data:
                self._audit_contract_id = ContractId(data["Ok"])
                return self._audit_contract_id
            raise RuntimeError(f"get_applet_address failed: {data.get('Err', data)}")

    async def _submit_audit(self, log: str) -> TransactionResult:
        """Submit an audit log entry to the blockchain."""
        contract_id = await self._get_audit_contract_id()
        method_args = json.dumps({"log": log})

        return await self.to_contract_client(contract_id).execute(
            "audit", method_args, False, True
        )

    def audit(self, log: Optional[str] = None) -> Any:
        """Submit an audit log entry, or use as a decorator factory.

        Direct call:   await client.audit("log string")
        Decorator:     @client.audit()   — prepends two lines to the wrapped
                       function: builds a JSON entry from all arguments, then
                       calls client._submit_audit(entry) before the handler runs.
        """
        if log is not None:
            return self._submit_audit(log)

        def decorator(func: Callable) -> Callable:
            sig = inspect.signature(func)
            positional_params = [
                name
                for name, p in sig.parameters.items()
                if p.kind
                in (
                    inspect.Parameter.POSITIONAL_OR_KEYWORD,
                    inspect.Parameter.POSITIONAL_ONLY,
                )
            ]

            @functools.wraps(func)
            async def wrapper(*args: Any, **kwargs: Any) -> Any:
                entry = json.dumps(dict(zip(positional_params, args)) | kwargs)
                await self._submit_audit(entry)
                return await func(*args, **kwargs)

            return wrapper

        return decorator

    async def close(self) -> None:
        """Close the HTTP client."""
        await self._http_client.aclose()

    async def __aenter__(self) -> "WeilClient":
        """Enter async context: pre-resolve the audit applet address."""
        await self._get_audit_contract_id()
        return self

    async def __aexit__(self, *args: Any) -> None:
        """Exit async context: close the underlying HTTP client."""
        await self.close()

    @staticmethod
    def _build_submit_payload(
        signature: str,
        base_txn: BaseTransaction,
        args: dict[str, Any],
    ) -> SubmitTxnRequest:
        """Build the SubmitTxnRequest with fresh creation_time."""
        h = base_txn.header
        req_header = TransactionHeader(
            nonce=h.nonce,
            public_key=h.public_key,
            from_addr=h.from_addr,
            to_addr=h.to_addr,
            signature=signature,
            weilpod_counter=h.weilpod_counter,
            creation_time=int(current_time_millis()),
        )
        user_txn = UserTransaction(
            ty="SmartContractExecutor",
            contract_address=args["contract_address"],
            contract_method=args["contract_method"],
            contract_input_bytes=args["contract_input_bytes"],
            should_hide_args=args["should_hide_args"],
        )
        txn = Transaction(
            is_xpod=False,
            txn_header=req_header,
            verifier=Verifier(),
            user_txn=user_txn,
        )
        return SubmitTxnRequest(transaction=txn)


class WeilContractClient:
    """Per-contract client for calling methods on a single applet."""

    def __init__(self, contract_id: ContractId, client: WeilClient) -> None:
        """Bind a WeilClient to a specific contract.

        Args:
            contract_id: The target applet's ContractId.
            client: The parent WeilClient supplying the wallet and HTTP connection.
        """
        self._contract_id = contract_id
        self._client = client

    def wallet_addr(self) -> str:
        """Return the wallet address of the underlying client."""
        return self._client.wallet_addr()

    def _sign_and_construct_txn(
        self, method_name: str, method_args: str, should_hide_args: bool
    ) -> tuple[BaseTransaction, str, dict]:
        """Build and sign the base transaction and execute args."""
        public_key = self._client._wallet.get_public_key()
        from_addr = get_address_from_public_key(public_key)
        to_addr = from_addr
        weilpod_counter = self._contract_id.pod_counter()
        # Rust uses full (uncompressed) for on-wire; parsed_public_key expects Full
        public_key_hex = public_key.format(compressed=False).hex()

        args = {
            "contract_address": self._contract_id,
            "contract_method": method_name,
            "contract_input_bytes": method_args,
            "should_hide_args": should_hide_args,
        }

        nonce = int(current_time_millis())
        header = TransactionHeader(
            nonce=nonce,
            public_key=public_key_hex,
            from_addr=from_addr,
            to_addr=to_addr,
            weilpod_counter=weilpod_counter,
        )

        signature = self._sign_execute_args(header, args)
        header.set_signature(signature)

        base_txn = BaseTransaction(header=header)
        return base_txn, signature, args

    def _sign_execute_args(
        self, txn_header: TransactionHeader, args: dict[str, Any]
    ) -> str:
        """Canonicalize and sign the execute payload.

        Match Rust: value_to_btreemap sorts only top-level keys; inner
        user_txn keeps json! insertion order (type, contract_address,
        contract_method, contract_input_bytes). Use same order so signature
        verifies on the server.
        """
        user_txn = {
            "type": "SmartContractExecutor",
            "contract_address": str(args["contract_address"]),
            "contract_method": args["contract_method"],
            "contract_input_bytes": args["contract_input_bytes"],
            "should_hide_args": args["should_hide_args"],
        }
        payload = {
            "from_addr": txn_header.from_addr,
            "nonce": txn_header.nonce,
            "to_addr": txn_header.to_addr,
            "user_txn": user_txn,
        }
        # Top-level sorted; use sort_keys=True so inner user_txn is also sorted
        # (server may canonicalize the same way when verifying)
        canonical = dict(sorted(payload.items()))
        json_str = json.dumps(canonical, separators=(",", ":"), sort_keys=True)
        return self._client._wallet.sign(json_str.encode("utf-8"))

    async def execute(
        self,
        method_name: str,
        method_args: str,
        should_hide_args: bool = True,
        is_non_blocking: bool = False,
    ) -> TransactionResult:
        """Execute an exported method (non-streaming)."""
        base_txn, signature, args = self._sign_and_construct_txn(
            method_name, method_args, should_hide_args
        )
        payload = WeilClient._build_submit_payload(signature, base_txn, args)

        async with self._client._semaphore:
            return await PlatformApi.submit_transaction(
                payload, self._client._http_client, is_non_blocking=is_non_blocking
            )

    async def execute_with_streaming(
        self,
        method_name: str,
        method_args: str,
    ) -> ByteStream:
        """Execute an exported method and return a streaming response."""
        base_txn, signature, args = self._sign_and_construct_txn(
            method_name, method_args, False
        )
        payload = WeilClient._build_submit_payload(signature, base_txn, args)

        async def stream() -> AsyncIterator[bytes]:
            async with self._client._semaphore:
                async for chunk in PlatformApi.submit_transaction_with_streaming(
                    payload, self._client._http_client, is_non_blocking=False
                ):
                    yield chunk

        return ByteStream(stream())
