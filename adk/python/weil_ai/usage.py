"""Framework-specific token usage extraction for audit payloads."""

from __future__ import annotations

from typing import Any, Optional


def langchain_usage_metadata(value: Any) -> Optional[Any]:
    """Return LangChain's documented AI message usage metadata."""
    try:
        from langchain_core.messages import AIMessage, AIMessageChunk
    except ImportError:
        return None

    if not isinstance(value, (AIMessage, AIMessageChunk)):
        return None
    return _jsonable_usage(value.usage_metadata)


def adk_usage_metadata(value: Any) -> Optional[Any]:
    """Return ADK / Google GenAI documented response usage metadata."""
    try:
        from google.adk.models.llm_response import LlmResponse
    except ImportError:
        LlmResponse = ()  # type: ignore[assignment]

    if LlmResponse and isinstance(value, LlmResponse):
        return _jsonable_usage(value.usage_metadata)

    try:
        from google.genai.types import (
            GenerateContentResponse,
            GenerateContentResponseUsageMetadata,
        )
    except ImportError:
        return None

    if isinstance(value, GenerateContentResponse):
        return _jsonable_usage(value.usage_metadata)
    if isinstance(value, GenerateContentResponseUsageMetadata):
        return _jsonable_usage(value)
    return None


def langchain_token_usage(value: Any) -> Optional[tuple[str, dict[str, int]]]:
    """Return ``(model, TokenUsage)`` for a LangChain AI message, or None."""
    usage = token_usage(langchain_usage_metadata(value))
    if usage is None:
        return None
    metadata = getattr(value, "response_metadata", None) or {}
    model = metadata.get("model_name") or metadata.get("model")
    return _model_name(model), usage


def adk_token_usage(value: Any) -> Optional[tuple[str, dict[str, int]]]:
    """Return ``(model, TokenUsage)`` for an ADK / GenAI response, or None."""
    usage = token_usage(adk_usage_metadata(value))
    if usage is None:
        return None
    return _model_name(getattr(value, "model_version", None)), usage


def token_usage(usage: Any) -> Optional[dict[str, int]]:
    """Normalize a framework usage dump into agent_registry's ``TokenUsage``.

    LangChain reports ``input_tokens`` / ``output_tokens``; ADK / GenAI
    reports prompt, tool-use prompt, candidate and thought counts, which are
    summed into the same two fields. Returns None when neither side has any
    tokens.
    """
    if not isinstance(usage, dict):
        return None

    def count(*keys: str) -> int:
        return sum(_int(usage.get(key)) for key in keys)

    input_tokens = count("input_tokens") or count(
        "prompt_token_count", "tool_use_prompt_token_count"
    )
    output_tokens = count("output_tokens") or count(
        "candidates_token_count", "thoughts_token_count"
    )
    if not (input_tokens or output_tokens):
        return None
    return {"input_tokens": input_tokens, "output_tokens": output_tokens}


def _int(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return 0
    return int(value)


def _model_name(value: Any) -> str:
    return value if isinstance(value, str) and value else "unknown"


def _jsonable_usage(value: Any) -> Optional[Any]:
    if value is None:
        return None
    if isinstance(value, dict):
        return {str(key): _jsonable_usage(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_jsonable_usage(item) for item in value]
    if isinstance(value, (str, int, float, bool)):
        return value
    if hasattr(value, "model_dump"):
        try:
            return value.model_dump(mode="json", exclude_none=True)
        except Exception:
            pass
    return repr(value)
