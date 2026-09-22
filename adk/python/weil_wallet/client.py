"""WeilClient and WeilContractClient for executing applet methods on WeilChain."""

from __future__ import annotations
import asyncio
import functools
import inspect
import json
import warnings
from typing import Any, AsyncIterator, Callable, Optional
import httpx
from .api.platform_api import PlatformApi
from .api.request import SubmitTxnRequest, Transaction, UserTransaction, Verifier
from .constants import DEFAULT_CONCURRENCY, SENTINEL_HOST
from .contract import ContractId
from .streaming import ByteStream
from .transaction import BaseTransaction, TransactionHeader, TransactionResult
from .utils import current_time_millis
from .wallet import SelectedAccount, Wallet

AUDIT_APPLET_SVC_NAME = "auditor::weil"

class WeilClient:
    """High-level client for WeilChain applet methods.

    Holds an HTTP client, a signer Wallet, and a concurrency limiter.
    """

    def __init__(
        self,
        wallet: Wallet,
        concurrency: Optional[int] = None,
        *,
        sentinel_host: Optional[str] = SENTINEL_HOST,
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
        self._verify = verify
        self._http_client = httpx.AsyncClient(
            base_url=self._sentinel_host.rstrip("/"),
            verify=verify,
            timeout=60.0,
        )
        self._audit_contract_id: Optional[ContractId] = None
        self._audit_contract_id_lock = asyncio.Lock()
        self._remote_signing = False
        self._credentials: Optional[dict] = None

    @staticmethod
    def _http_client(
        sentinel_host: Optional[str] = SENTINEL_HOST,
        verify: bool = True,
    ) -> httpx.AsyncClient:
        return httpx.AsyncClient(
            base_url=(sentinel_host or SENTINEL_HOST).rstrip("/"),
            verify=verify,
            timeout=60.0,
        )

    @classmethod
    def from_wallet_file(
        cls,
        path: Any,
        concurrency: Optional[int] = None,
        *,
        secure: bool = False,
        require_lock: bool = True,
        harden_process: bool = True,
        sentinel_host: Optional[str] = SENTINEL_HOST,
        verify: bool = True,
    ) -> "WeilClient":
        """Create a WeilClient from a wallet.wc file.

        Convenience wrapper around Wallet.from_wallet_file + WeilClient().
        No sentinel connection required for wallet construction.

        Args:
            path: Path to the wallet.wc file.
            concurrency: Max concurrent in-flight requests.
            sentinel_host: Base URL of the Sentinel node.
            verify: Whether to verify TLS certificates.
        """
        wallet = Wallet.from_wallet_file(
            path,
            secure=secure,
            require_lock=require_lock,
            harden_process=harden_process,
        )
        return cls(wallet, concurrency, sentinel_host=sentinel_host, verify=verify)

    @classmethod
    async def from_api_key(
        cls,
        api_key: str,
        concurrency: Optional[int] = None,
        *,
        sentinel_host: Optional[str] = SENTINEL_HOST,
        creds: Optional[dict] = None,
        verify: bool = True,
        secure: bool = False,
        require_lock: bool = True,
        harden_process: bool = True,
        include_creds: bool = False,
    ) -> "WeilClient":
        """Create a WeilClient from an Agent Registry API key.

        The endpoint always returns a masked wallet, so signing is delegated to
        the Sentinel. ``secure=True`` fetches the unmasked wallet over native
        HTTPS into secure memory — the secret key is derived in mlocked memory
        and signing is done locally, same as a wallet file with ``secure=True``.
        ``creds`` must not be passed in that case (AWS credentials are read from
        environment variables natively).

        Args:
            include_creds: With ``secure=True``, attach env-var AWS credentials
                to the wallet fetch. Set True only for external wallets stored
                under those creds.
        """
        if secure:
            if creds is not None:
                warnings.warn(
                    "creds= is ignored with secure=True; the secure native "
                    "backend reads AWS credentials from the environment. "
                    "Attaching env-var creds to the request.",
                    stacklevel=2,
                )
                include_creds = True
            wallet = await asyncio.to_thread(
                Wallet.from_secure_api_key,
                api_key,
                sentinel_host=sentinel_host or SENTINEL_HOST,
                verify=verify,
                require_lock=require_lock,
                harden_process=harden_process,
                include_creds=include_creds,
            )
        else:
            wallet = await cls.get_agent_wallet(
                api_key,
                sentinel_host=sentinel_host,
                creds=creds,
                verify=verify,
            )
        client = cls(wallet, concurrency, sentinel_host=sentinel_host, verify=verify)
        if not secure:
            client._remote_signing = True
            client._credentials = creds
        return client

    # ── Multi-account management ───────────────────────────────────────────────

    async def set_account(self, selected: SelectedAccount) -> None:
        """Switch the active account used for signing.

        Also drops the cached audit contract id — it's pinned to whichever
        account's home pod resolved it (see _get_audit_contract_id), so a stale
        entry from the previous account must not leak into calls made under the
        new one.

        Raises:
            IndexError: If the index is out of bounds.
        """
        self._wallet.set_index(selected)
        async with self._audit_contract_id_lock:
            self._audit_contract_id = None

    async def derived_account_count(self) -> int:
        """Return the number of BIP32 HD-derived accounts in the wallet."""
        return self._wallet.derived_account_count()

    async def external_account_count(self) -> int:
        """Return the number of externally added accounts in the wallet."""
        return self._wallet.external_account_count()

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

    @classmethod
    async def get_agent_wallet(
        cls,
        api_key: str,
        *,
        sentinel_host: Optional[str] = SENTINEL_HOST,
        creds: Optional[dict] = None,
        verify: bool = True,
    ) -> Wallet:
        """Fetch the masked wallet registered for an Agent Registry API key.

        POSTs ``{"api_key", "unmasked": false, "credentials"}`` to Sentinel's
        ``/get_agent_wallet`` endpoint and returns the resulting public-only
        :class:`Wallet` (signing is delegated to Sentinel).

        Failures come back as ``(status, {"status": "failure", "message": ...})``;
        only that ``message`` is carried into the raised :class:`RuntimeError`,
        so the wallet cannot leak through an exception.

        Args:
            api_key: Agent Registry API key identifying the wallet.
            sentinel_host: Base URL of the Sentinel node.
            creds: Storage credentials (e.g. AWS S3 keys) for wallets stored
                in a caller-owned bucket.
            verify: Whether to verify TLS certificates.
        """
        payload: dict[str, Any] = {"api_key": api_key}
        payload["unmasked"] = False
        if creds:
            payload["credentials"] = creds

        async with cls._http_client(sentinel_host, verify) as http_client:
            resp = await http_client.post(
                "/get_agent_wallet",
                json=payload,
            )


        if not resp.is_success:
            try:
                error = resp.json()
            except json.JSONDecodeError:
                error = None
            message = error.get("message") if isinstance(error, dict) else "no message"
            raise RuntimeError(
                f"agent wallet lookup failed: HTTP {resp.status_code}: {message}"
            )

        try:
            result = resp.json()
        except json.JSONDecodeError:
            body = resp.text.strip()
            raise RuntimeError(
                f"agent wallet lookup failed: non-JSON response "
                f"(HTTP {resp.status_code}, {len(body)} bytes)"
            )


        if isinstance(result, dict) and result.get("type") == "wallet":
            return Wallet.from_wallet_json(result, masked=True)

        if isinstance(result, str):
            wallet_json = result.strip()
            if not wallet_json:
                raise RuntimeError("agent wallet not found for API key")
            return Wallet.from_wallet_json(wallet_json, masked=True)

        raise RuntimeError(
            f"agent wallet lookup failed: unexpected response type {type(result).__name__}"
        )

    def wallet_addr(self) -> str:
        """Return the sentinel-minted 72-char hex account address."""
        return self._wallet.get_address()

    async def sign_message(self, payload: dict[str, Any]) -> str:
        """Sign a canonical JSON message via the Sentinel.

        Used for transaction and MCP auth-header signing on non-secure
        API-key clients. Secure clients sign locally and use sign() directly.
        """
        if not self._remote_signing:
            raise RuntimeError(
                "sign_message requires an API-key client (from_api_key)"
            )

        if not self._wallet._source_json:
            raise RuntimeError("remote signing credentials are unavailable")
        req = {
            "payload": payload,
            "wallet": json.loads(self._wallet._source_json),
        }
        if self._credentials:
            req["credentials"] = self._credentials

        resp = await self._http_client.post("/sign_payload", json=req)
        resp.raise_for_status()
        try:
            data = resp.json()
        except json.JSONDecodeError:
            body = resp.text.strip()
            if len(body) == 128:
                try:
                    bytes.fromhex(body)
                except ValueError:
                    pass
                else:
                    return body
            raise RuntimeError(
                f"remote signing failed: non-JSON response "
                f"(HTTP {resp.status_code}): {body}"
            )
        if not isinstance(data, dict):
            raise RuntimeError(
                f"remote signing failed: unexpected response type {type(data).__name__}"
            )

        signature = data.get("signature")
        if signature is None:
            raise RuntimeError("remote signing failed: response missing signature")
        return signature

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
        """Resolve and cache the audit applet contract address from the Sentinel API.

        Sends the caller's own ``wallet_address`` alongside ``svc_name`` so
        Sentinel pins resolution to that wallet's home weilpod (derived
        server-side from the pod counter embedded in the address) instead of
        falling back to a random pod in its region. Without this,
        ``validate_and_persist_receipt`` can land on a pod whose local
        ``identity::<org>`` copy never saw this wallet's membership.
        """
        async with self._audit_contract_id_lock:
            if self._audit_contract_id is not None:
                return self._audit_contract_id
            resp = await self._http_client.post(
                "/get_applet_address",
                json={
                    "svc_name": AUDIT_APPLET_SVC_NAME,
                    "wallet_address": self.wallet_addr(),
                },
            )
            resp.raise_for_status()
            data = resp.json()
            if "Ok" in data:
                self._audit_contract_id = ContractId(data["Ok"])
                return self._audit_contract_id
            raise RuntimeError(f"get_applet_address failed: {data.get('Err', data)}")

    async def get_receipt_for_commit(self, commit_hash: str) -> Optional[str]:
        """Read back the receipt content currently persisted for ``commit_hash``.

        Returns None if nothing has been persisted for it yet.

        Used by the same-turn-commit-gap amend path to fetch the payload an
        agent-run mid-turn commit already shipped — with empty prompts/usage,
        since that commit landed before Stop had computed them — so it can be
        merged and re-persisted under the same commit hash.

        Raises:
            RuntimeError: If the applet returned an error, or the response
                could not be decoded.
        """
        contract_id = await self._get_audit_contract_id()
        method_args = json.dumps({"commit_hash": commit_hash})
        resp = await self.execute(
            contract_id, "get_receipt_for_commit", method_args, False, False
        )
        return self._parse_get_receipt_for_commit_result(resp.txn_result)

    @staticmethod
    def _parse_get_receipt_for_commit_result(txn_result: str) -> Optional[str]:
        """Decode the txn_result of a get_receipt_for_commit call.

        Returns the receipt content, or None when nothing is persisted for the
        commit.

        The value is double-wrapped: the platform puts every contract call's
        result in an ``{"Ok": ...}``/``{"Err": ...}`` envelope, and "Ok"'s value
        is itself the callee's return value *re-serialized to a JSON string*
        rather than embedded directly. So this needs two decode passes: unwrap
        the envelope, then parse the resulting string to reach the actual
        optional receipt.
        """
        try:
            envelope = json.loads(txn_result)
        except ValueError as e:
            raise RuntimeError(
                f"failed to parse get_receipt_for_commit response: {e}"
            ) from e

        ok_value = envelope
        if isinstance(envelope, dict):
            if "Err" in envelope:
                raise RuntimeError(
                    f"get_receipt_for_commit returned an error: {envelope['Err']}"
                )
            if "Ok" in envelope:
                ok_value = envelope["Ok"]

        inner = ok_value
        if ok_value is None:
            return None
        if isinstance(ok_value, str):
            try:
                inner = json.loads(ok_value)
            except ValueError as e:
                raise RuntimeError(
                    f"failed to parse get_receipt_for_commit inner value: {e}"
                ) from e

        if inner is None:
            return None
        if isinstance(inner, str):
            return inner
        raise RuntimeError(f"unexpected get_receipt_for_commit payload shape: {inner}")

    async def _submit_audit(self, log: str) -> TransactionResult:
        """Submit an audit log entry to the blockchain."""
        contract_id = await self._get_audit_contract_id()
        org = self._wallet.org
        org_name = org.name if org else None
        subgroup = org.subgroup if org else None
        method_args = json.dumps({"log": log, "org": org_name, "subgroup": subgroup})

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

            @functools.wraps(func)
            async def wrapper(*args: Any, **kwargs: Any) -> Any:
                bound = sig.bind(*args, **kwargs)
                payload = {
                    name: value
                    for name, value in bound.arguments.items()
                    if name not in ("self", "cls")
                }
                await self._submit_audit(json.dumps(payload, default=repr))
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
            salt=h.salt,
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

    async def _sign_and_construct_txn(
        self, method_name: str, method_args: str, should_hide_args: bool
    ) -> tuple[BaseTransaction, str, dict]:
        """Build and sign the base transaction and execute args."""
        public_key = self._client._wallet.get_public_key()
        from_addr = self._client._wallet.get_address()
        to_addr = from_addr
        weilpod_counter = self._contract_id.pod_counter()
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

        signature = await self._sign_execute_args(header, args)
        header.set_signature(signature)

        base_txn = BaseTransaction(header=header)
        return base_txn, signature, args

    async def _sign_execute_args(
        self, txn_header: TransactionHeader, args: dict[str, Any]
    ) -> str:
        """Canonicalize and sign the execute payload."""
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
            "salt": txn_header.salt,
            "to_addr": txn_header.to_addr,
            "user_txn": user_txn,
        }
        canonical = dict(sorted(payload.items()))
        json_str = json.dumps(canonical, separators=(",", ":"), sort_keys=True)

        if self._client._remote_signing:
            return await self._client.sign_message(payload)
        return self._client._wallet.sign(json_str.encode("utf-8"))

    async def execute(
        self,
        method_name: str,
        method_args: str,
        should_hide_args: bool = True,
        is_non_blocking: bool = False,
    ) -> TransactionResult:
        """Execute an exported applet method and return the transaction result.

        Builds and signs the transaction, then submits it to the platform API.
        Concurrency is bounded by the parent client's semaphore.

        Args:
            method_name:      The exported method to invoke.
            method_args:      JSON-encoded argument payload.
            should_hide_args: When True the arguments are encrypted before submission.
            is_non_blocking:  When True the platform responds immediately without
                              waiting for transaction finalization.

        Returns:
            TransactionResult with status, block height, and application result.
        """
        base_txn, signature, args = await self._sign_and_construct_txn(
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
        """Execute an exported applet method and return an async streaming response.

        Suitable for methods that produce incremental output (e.g. LLM inference).
        Iterate the returned ByteStream with ``async for chunk in stream``.

        Args:
            method_name: The exported method to invoke.
            method_args: JSON-encoded argument payload.

        Returns:
            ByteStream that yields ``bytes`` chunks as they arrive.
        """
        base_txn, signature, args = await self._sign_and_construct_txn(
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
