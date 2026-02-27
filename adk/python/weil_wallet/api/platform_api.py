"""Platform API: submit transaction (normal and streaming)."""

from typing import AsyncIterator

import httpx

from ..transaction import TransactionResult
from ..utils import compress
from .request import SubmitTxnRequest


class PlatformApi:
    """Submit transactions to the WeilChain platform."""

    @staticmethod
    async def _submit_transaction_inner(
        payload: SubmitTxnRequest,
        client: httpx.AsyncClient,
        *,
        is_non_blocking: bool,
    ) -> httpx.Response:
        """GZIP-compress and POST the transaction payload; raise on HTTP error."""
        payload_dict = payload.to_payload_dict()
        tx_payload = compress(payload_dict)

        files = {
            "transaction": ("transaction_data", tx_payload, "application/octet-stream")
        }

        headers = {}

        if is_non_blocking:
            headers["x-non-blocking"] = "true"

        response = await client.post(
            "/contracts/execute_smartcontract", files=files, headers=headers
        )

        if not response.is_success:
            body = response.text[:500] if response.text else ""

            raise RuntimeError(
                f"failed to submit the transaction: HTTP {response.status_code} {body}"
            )

        return response

    @staticmethod
    async def submit_transaction(
        payload: SubmitTxnRequest,
        client: httpx.AsyncClient,
        *,
        is_non_blocking: bool = False,
    ) -> TransactionResult:
        """Submit and return the parsed TransactionResult."""
        response = await PlatformApi._submit_transaction_inner(
            payload, client, is_non_blocking=is_non_blocking
        )
        data = response.json()
        return TransactionResult.from_dict(data)

    @staticmethod
    async def submit_transaction_with_streaming(
        payload: SubmitTxnRequest,
        client: httpx.AsyncClient,
        *,
        is_non_blocking: bool = False,
    ) -> AsyncIterator[bytes]:
        """Submit and return an async iterator of response body chunks."""
        response = await PlatformApi._submit_transaction_inner(
            payload, client, is_non_blocking=is_non_blocking
        )
        async for chunk in response.aiter_bytes():
            if chunk:
                yield chunk
