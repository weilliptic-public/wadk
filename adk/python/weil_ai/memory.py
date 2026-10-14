"""Framework tool adapters for the Pensieve graph-memory applet."""

from __future__ import annotations

import json
from typing import Annotated, Any, Callable, Optional

from weil_wallet.pensieve import Pensieve
from weil_wallet.transaction import TransactionResult



PENSIEVE_TOOL_NAMES = frozenset(
    {
        "remember_entity",
        "remember_relation",
        "remember_batch",
        "add_observations",
        "forget_observations",
        "forget_entities",
        "forget_relation",
        "recall_entity",
        "search_memory",
        "traverse",
        "list_relation_types",
        "recall_graph",
    }
)

EntityNameArg = Annotated[
    str,
    'The name of the entity, like "Olive Garden" or "self". Use "self" for facts about the agent or the user it is acting for.',
]
EntityTypeArg = Annotated[
    str,
    'The category of the entity, like "person", "restaurant", "company", "project" or "preference". Use a short, reusable noun.',
]
ObservationsArg = Annotated[
    list[str],
    "Atomic properties that belong to this entity alone and name nothing else. Put related things in entities plus relations, not observations.",
]
SourceArg = Annotated[
    str,
    'The name of the entity the relation starts at, like "self".',
]
RelationArg = Annotated[
    str,
    'The predicate, in active voice, like "likes_restaurant" or "works_at". Reuse predicates across entities so the graph stays queryable.',
]
TargetArg = Annotated[
    str,
    'The name of the entity the relation points to, like "Olive Garden".',
]
GraphJsonArg = Annotated[
    str,
    'A JSON object with optional "entities" and "relations" arrays. Include every entity a relation references, including "self"; observations may be omitted.',
]
NamesArg = Annotated[
    list[str],
    "The names of the entities to delete.",
]
SearchTextArg = Annotated[
    str,
    'The text to look for, like "restaurant" or "olive". Matching is case-insensitive substring matching, not semantic search.',
]
LimitArg = Annotated[
    Optional[int],
    "Maximum number of entities to return. Check the truncated field on graph results before treating the result as complete.",
]
StartArg = Annotated[
    str,
    "The name of the entity to start walking from.",
]
OptionalRelationArg = Annotated[
    Optional[str],
    'Only follow this predicate, like "works_at". Omit to follow every predicate.',
]
DirectionArg = Annotated[
    Optional[str],
    '"out" to follow relations forward, "in" backward, or "both" to ignore direction. Defaults to "out".',
]
DepthArg = Annotated[
    Optional[int],
    "How many hops to walk, from 1 to 5. Defaults to 2.",
]


def make_langchain_pensieve_tools(owner: Any) -> list[Any]:
    """Return LangChain tools bound to *owner*'s wallet-scoped Pensieve memory."""

    try:
        from langchain_core.tools import StructuredTool
    except ImportError as exc:
        raise ImportError(
            "langchain-core is required for LangChain Pensieve tools"
        ) from exc

    return [
        StructuredTool.from_function(
            func=fn,
            coroutine=afn,
            name=fn.__name__,
            description=fn.__doc__ or "",
        )
        for fn, afn in _pensieve_functions(owner)
    ]


def make_adk_pensieve_tools(owner: Any) -> list[Any]:
    """Return Google ADK tools bound to *owner*'s wallet-scoped Pensieve memory."""

    async_functions = [afn for _, afn in _pensieve_functions(owner)]
    try:
        from google.adk.tools import FunctionTool
    except ImportError:
        return async_functions

    tools = []
    for fn in async_functions:
        try:
            tools.append(FunctionTool(fn))
        except TypeError:
            tools.append(FunctionTool(func=fn))
    return tools


def append_unique_tools(existing: Any, additions: list[Any]) -> list[Any]:
    """Append tools by name without duplicating caller-provided tools."""

    tools = list(existing or [])
    names = {_tool_name(tool) for tool in tools}
    for tool in additions:
        name = _tool_name(tool)
        if name not in names:
            tools.append(tool)
            names.add(name)
    return tools


def _pensieve_functions(owner: Any) -> list[tuple[Callable[..., Any], Callable[..., Any]]]:
    pensieve = Pensieve(owner)

    async def aremember_entity(
        name: str,
        entity_type: str,
        observations: list[str],
    ) -> Any:
        """Write one entity into this agent's memory, creating it if it is new. The entity keeps existing observations, gains new observations, and takes the new name and type; write entities before relating them."""
        return _unwrap_result(
            await pensieve.remember_entity(
                _agent_id(owner),
                name,
                entity_type,
                observations,
            )
        )

    def remember_entity(
        name: EntityNameArg,
        entity_type: EntityTypeArg,
        observations: ObservationsArg,
    ) -> Any:
        """Write one entity into this agent's memory, creating it if it is new. The entity keeps existing observations, gains new observations, and takes the new name and type; write entities before relating them."""
        return _run_sync(
            aremember_entity(
                name=name,
                entity_type=entity_type,
                observations=observations,
            )
        )

    async def aremember_relation(source: str, relation: str, target: str) -> Any:
        """Connect two existing entities in this agent's memory with a directed relation. Writing a relation that already exists is a no-op that returns the original."""
        return _unwrap_result(
            await pensieve.remember_relation(
                _agent_id(owner),
                source,
                relation,
                target,
            )
        )

    def remember_relation(
        source: SourceArg,
        relation: RelationArg,
        target: TargetArg,
    ) -> Any:
        """Connect two existing entities in this agent's memory with a directed relation. Writing a relation that already exists is a no-op that returns the original."""
        return _run_sync(
            aremember_relation(source=source, relation=relation, target=target)
        )

    async def aremember_batch(graph_json: str) -> Any:
        """Write many entities and relations in one call. Use a JSON object with optional entities and relations arrays; entities are written first, so relations in the same payload may reference them."""
        return _unwrap_result(
            await pensieve.remember_batch(_agent_id(owner), graph_json)
        )

    def remember_batch(graph_json: GraphJsonArg) -> Any:
        """Write many entities and relations in one call. Use a JSON object with optional entities and relations arrays; entities are written first, so relations in the same payload may reference them."""
        return _run_sync(aremember_batch(graph_json=graph_json))

    async def aadd_observations(entity: str, observations: list[str]) -> Any:
        """Add observations to an entity that already exists in this agent's memory. Observations already recorded are ignored, so this is safe to call repeatedly."""
        return _unwrap_result(
            await pensieve.add_observations(
                _agent_id(owner),
                entity,
                observations,
            )
        )

    def add_observations(
        entity: EntityNameArg,
        observations: ObservationsArg,
    ) -> Any:
        """Add observations to an entity that already exists in this agent's memory. Observations already recorded are ignored, so this is safe to call repeatedly."""
        return _run_sync(
            aadd_observations(entity=entity, observations=observations)
        )

    async def aforget_observations(entity: str, observations: list[str]) -> Any:
        """Remove exact observations from an entity, leaving the entity and its relations in place. Use this when a fact is superseded rather than deleting the whole entity."""
        return _unwrap_result(
            await pensieve.forget_observations(
                _agent_id(owner),
                entity,
                observations,
            )
        )

    def forget_observations(
        entity: EntityNameArg,
        observations: ObservationsArg,
    ) -> Any:
        """Remove exact observations from an entity, leaving the entity and its relations in place. Use this when a fact is superseded rather than deleting the whole entity."""
        return _run_sync(
            aforget_observations(entity=entity, observations=observations)
        )

    async def aforget_entities(names: list[str]) -> Any:
        """Delete entities from this agent's memory and every relation touching them, in either direction."""
        return _unwrap_result(
            await pensieve.forget_entities(_agent_id(owner), names)
        )

    def forget_entities(names: NamesArg) -> Any:
        """Delete entities from this agent's memory and every relation touching them, in either direction."""
        return _run_sync(aforget_entities(names=names))

    async def aforget_relation(source: str, relation: str, target: str) -> Any:
        """Delete one relation from this agent's memory, leaving both endpoint entities in place. Returns false when the relation was not in memory."""
        return _unwrap_result(
            await pensieve.forget_relation(
                _agent_id(owner),
                source,
                relation,
                target,
            )
        )

    def forget_relation(
        source: SourceArg,
        relation: RelationArg,
        target: TargetArg,
    ) -> Any:
        """Delete one relation from this agent's memory, leaving both endpoint entities in place. Returns false when the relation was not in memory."""
        return _run_sync(
            aforget_relation(source=source, relation=relation, target=target)
        )

    async def arecall_entity(name: str) -> Any:
        """Read one entity from this agent's memory together with every relation that touches it. This is the cheapest way to answer what is known about one entity."""
        return _unwrap_result(await pensieve.recall_entity(_agent_id(owner), name))

    def recall_entity(name: EntityNameArg) -> Any:
        """Read one entity from this agent's memory together with every relation that touches it. This is the cheapest way to answer what is known about one entity."""
        return _run_sync(arecall_entity(name=name))

    async def asearch_memory(text: str, limit: Optional[int] = None) -> Any:
        """Find entities whose name, type, or observations contain the given text, together with relations running between the matches. Matching is case-insensitive substring matching, not semantic search."""
        return _unwrap_result(
            await pensieve.search_memory(_agent_id(owner), text, limit)
        )

    def search_memory(text: SearchTextArg, limit: LimitArg = None) -> Any:
        """Find entities whose name, type, or observations contain the given text, together with relations running between the matches. Matching is case-insensitive substring matching, not semantic search."""
        return _run_sync(asearch_memory(text=text, limit=limit))

    async def atraverse(
        start: StartArg,
        relation: OptionalRelationArg = None,
        direction: DirectionArg = None,
        depth: DepthArg = None,
    ) -> Any:
        """Walk this agent's memory graph outward from one entity and return everything reachable within depth hops, with connecting relations. Use this when a question spans related things."""
        return _unwrap_result(
            await pensieve.traverse(
                _agent_id(owner),
                start,
                relation,
                direction,
                depth,
            )
        )

    def traverse(
        start: str,
        relation: Optional[str] = None,
        direction: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> Any:
        """Walk this agent's memory graph outward from one entity and return everything reachable within depth hops, with connecting relations. Use this when a question spans related things."""
        return _run_sync(
            atraverse(
                start=start,
                relation=relation,
                direction=direction,
                depth=depth,
            )
        )

    async def alist_relation_types(name: str) -> Any:
        """List the distinct predicates recorded on an entity in this agent's memory, in either direction. Call this before traverse to discover useful relations."""
        return _unwrap_result(
            await pensieve.list_relation_types(_agent_id(owner), name)
        )

    def list_relation_types(name: EntityNameArg) -> Any:
        """List the distinct predicates recorded on an entity in this agent's memory, in either direction. Call this before traverse to discover useful relations."""
        return _run_sync(alist_relation_types(name=name))

    async def arecall_graph(limit: Optional[int] = None) -> Any:
        """Read this agent's whole memory graph. Prefer search_memory or traverse once memory is large, and always check the truncated field before treating the result as complete."""
        return _unwrap_result(await pensieve.recall_graph(_agent_id(owner), limit))

    def recall_graph(limit: LimitArg = None) -> Any:
        """Read this agent's whole memory graph. Prefer search_memory or traverse once memory is large, and always check the truncated field before treating the result as complete."""
        return _run_sync(arecall_graph(limit=limit))

    pairs = [
        (remember_entity, aremember_entity),
        (remember_relation, aremember_relation),
        (remember_batch, aremember_batch),
        (add_observations, aadd_observations),
        (forget_observations, aforget_observations),
        (forget_entities, aforget_entities),
        (forget_relation, aforget_relation),
        (recall_entity, arecall_entity),
        (search_memory, asearch_memory),
        (traverse, atraverse),
        (list_relation_types, alist_relation_types),
        (recall_graph, arecall_graph),
    ]
    for sync_fn, async_fn in pairs:
        _copy_tool_metadata(sync_fn, async_fn)
    return pairs


def _copy_tool_metadata(sync_fn: Callable[..., Any], async_fn: Callable[..., Any]) -> None:
    async_fn.__name__ = sync_fn.__name__
    async_fn.__qualname__ = sync_fn.__qualname__
    async_fn.__doc__ = sync_fn.__doc__
    async_fn.__annotations__ = dict(sync_fn.__annotations__)


def _agent_id(owner: Any) -> str:
    return _client(owner).wallet_addr()


def _client(owner: Any) -> Any:
    get_client = getattr(owner, "_get_client", None)
    if callable(get_client):
        return get_client()
    return owner


def _unwrap_result(result: Any) -> Any:
    if not isinstance(result, TransactionResult):
        return result
    if not result.txn_result:
        return result
    try:
        envelope = json.loads(result.txn_result)
    except ValueError:
        return result.txn_result
    if isinstance(envelope, dict):
        if "Err" in envelope:
            raise RuntimeError(str(envelope["Err"]))
        if "Ok" in envelope:
            return _maybe_parse_json(envelope["Ok"])
    return envelope


def _maybe_parse_json(value: Any) -> Any:
    if not isinstance(value, str):
        return value
    try:
        return json.loads(value)
    except ValueError:
        return value


def _run_sync(coro: Any) -> Any:
    from .mcp import _run_sync as run_sync

    return run_sync(coro)


def _tool_name(tool: Any) -> str:
    return str(
        getattr(tool, "name", None)
        or getattr(tool, "__name__", None)
        or getattr(getattr(tool, "func", None), "__name__", None)
        or ""
    )
