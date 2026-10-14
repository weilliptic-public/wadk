"""Pensieve graph-memory applet helpers for Weil agents."""

from __future__ import annotations

import asyncio
import json
from typing import Any, Mapping, Optional, Union

from .client import WeilClient
from .constants import PENSIEVE_APPLET_SVC_NAME
from .contract import ContractId
from .transaction import TransactionResult


class Pensieve:
    """Client for the Pensieve graph-memory applet.

    The wrapper accepts either a :class:`weil_wallet.WeilClient` or a
    :class:`weil_ai.WeilAgent`-like object with ``_get_client()``. Calls are
    submitted through the same transaction path used by ``WeilClient.audit``.
    """

    def __init__(
        self,
        owner: Any,
        *,
        svc_name: str = PENSIEVE_APPLET_SVC_NAME,
        contract_id: Optional[Union[str, ContractId]] = None,
    ) -> None:
        self._client = _coerce_client(owner)
        self._svc_name = svc_name
        self._contract_id_explicit = contract_id is not None
        self._contract_id = (
            ContractId(contract_id) if isinstance(contract_id, str) else contract_id
        )
        self._contract_wallet_addr: Optional[str] = None
        self._contract_id_lock = asyncio.Lock()

    async def _get_contract_id(self) -> ContractId:
        async with self._contract_id_lock:
            if self._contract_id_explicit and self._contract_id is not None:
                return self._contract_id
            wallet_addr = self._client.wallet_addr()
            if (
                self._contract_id is not None
                and self._contract_wallet_addr == wallet_addr
            ):
                return self._contract_id
            self._contract_id = await WeilClient.get_applet_id_for_name(
                self._client._http_client,
                self._svc_name,
                wallet_address=wallet_addr,
            )
            self._contract_wallet_addr = wallet_addr
            return self._contract_id

    async def _execute(
        self,
        method_name: str,
        method_args: Mapping[str, Any],
    ) -> TransactionResult:
        contract_id = await self._get_contract_id()
        return await self._client.to_contract_client(contract_id).execute(
            method_name,
            json.dumps(method_args),
            False,
            False,
        )

    async def remember_entity(
        self,
        agent_id: str,
        name: str,
        entity_type: str,
        observations: list[str],
    ) -> TransactionResult:
        return await self._execute(
            "remember_entity",
            {
                "agent_id": agent_id,
                "name": name,
                "entity_type": entity_type,
                "observations": observations,
            },
        )

    async def remember_relation(
        self,
        agent_id: str,
        source: str,
        relation: str,
        target: str,
    ) -> TransactionResult:
        return await self._execute(
            "remember_relation",
            {
                "agent_id": agent_id,
                "source": source,
                "relation": relation,
                "target": target,
            },
        )

    async def remember_batch(
        self,
        agent_id: str,
        graph_json: Union[str, Mapping[str, Any]],
    ) -> TransactionResult:
        if not isinstance(graph_json, str):
            graph_json = json.dumps(graph_json)
        return await self._execute(
            "remember_batch",
            {"agent_id": agent_id, "graph_json": graph_json},
        )

    async def add_observations(
        self,
        agent_id: str,
        entity: str,
        observations: list[str],
    ) -> TransactionResult:
        return await self._execute(
            "add_observations",
            {
                "agent_id": agent_id,
                "entity": entity,
                "observations": observations,
            },
        )

    async def forget_observations(
        self,
        agent_id: str,
        entity: str,
        observations: list[str],
    ) -> TransactionResult:
        return await self._execute(
            "forget_observations",
            {
                "agent_id": agent_id,
                "entity": entity,
                "observations": observations,
            },
        )

    async def forget_entities(
        self,
        agent_id: str,
        names: list[str],
    ) -> TransactionResult:
        return await self._execute(
            "forget_entities",
            {"agent_id": agent_id, "names": names},
        )

    async def forget_relation(
        self,
        agent_id: str,
        source: str,
        relation: str,
        target: str,
    ) -> TransactionResult:
        return await self._execute(
            "forget_relation",
            {
                "agent_id": agent_id,
                "source": source,
                "relation": relation,
                "target": target,
            },
        )

    async def recall_entity(
        self,
        agent_id: str,
        name: str,
    ) -> TransactionResult:
        return await self._execute(
            "recall_entity",
            {"agent_id": agent_id, "name": name},
        )

    async def search_memory(
        self,
        agent_id: str,
        text: str,
        limit: Optional[int] = None,
    ) -> TransactionResult:
        return await self._execute(
            "search_memory",
            {"agent_id": agent_id, "text": text, "limit": limit},
        )

    async def traverse(
        self,
        agent_id: str,
        start: str,
        relation: Optional[str] = None,
        direction: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> TransactionResult:
        return await self._execute(
            "traverse",
            {
                "agent_id": agent_id,
                "start": start,
                "relation": relation,
                "direction": direction,
                "depth": depth,
            },
        )

    async def list_relation_types(
        self,
        agent_id: str,
        name: str,
    ) -> TransactionResult:
        return await self._execute(
            "list_relation_types",
            {"agent_id": agent_id, "name": name},
        )

    async def recall_graph(
        self,
        agent_id: str,
        limit: Optional[int] = None,
    ) -> TransactionResult:
        return await self._execute(
            "recall_graph",
            {"agent_id": agent_id, "limit": limit},
        )


def _coerce_client(owner: Any) -> WeilClient:
    if isinstance(owner, WeilClient):
        return owner
    get_client = getattr(owner, "_get_client", None)
    if callable(get_client):
        client = get_client()
        if isinstance(client, WeilClient):
            return client
    raise TypeError("Pensieve requires a WeilClient or WeilAgent-like owner")
