#!/usr/bin/env python3
"""Run a LangChain agent and verify Weil tool tracing calls add_trace."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any

EXAMPLE_DIR = Path(__file__).resolve().parent
REPO_ROOT = EXAMPLE_DIR.parent
sys.path = [path for path in sys.path if Path(path or ".").resolve() != EXAMPLE_DIR]
sys.path.insert(0, str(REPO_ROOT))

import weil_ai
import weil_ai.langchain as weil_langchain
import weil_ai.tracer as weil_tracer
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI


OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
MODEL = "gpt-5.1"
VERIFY_SENTINEL_TLS = False

WEIL_IDENTITIES = [
    "weil_xxx",
    "weil_xxx"
]


@tool
def get_store_details() -> str:
    """Return store metadata, including the user id needed to list items."""
    print(">>> TOOL: get_store_details()")
    return json.dumps(
        {
            "store_id": "store_001",
            "store_name": "Weil Demo Store",
            "user_id": "user_123",
        }
    )


@tool
def get_items(user_id: str) -> str:
    """Return the items visible to a given user id."""
    print(f">>> TOOL: get_items(user_id={user_id!r})")
    return json.dumps(
        [
            {"sku": "sku_coffee", "name": "Coffee Beans"},
            {"sku": "sku_tea", "name": "Green Tea"},
            {"sku": "sku_mug", "name": "Ceramic Mug"},
        ]
    )


@tool
def get_item_price(sku: str) -> str:
    """Return the current price for one item SKU."""
    print(f">>> TOOL: get_item_price(sku={sku!r})")
    prices = {
        "sku_coffee": "$14.00",
        "sku_tea": "$9.50",
        "sku_mug": "$12.00",
    }
    return json.dumps({"sku": sku, "price": prices.get(sku, "unknown")})


def install_trace_logger() -> None:
    original_initialize_trace = weil_tracer.initialize_trace
    original_build_trace = weil_tracer.build_trace
    original_get_trace = weil_tracer.get_trace
    original_publish_trace = weil_tracer.publish_trace

    async def logging_initialize_trace(owner: Any, state: weil_tracer.TraceState) -> Any:
        client = owner._get_client()
        org = getattr(getattr(client, "_wallet", None), "org", None)
        org_name = getattr(org, "name", "<missing-org>")
        print(f"\n>>> TRACE: calling tracer::{org_name}.add_trace")
        print(json.dumps({"trace_id": state.trace_id, "root": state.root, "trace": []}, indent=2))
        try:
            result = await original_initialize_trace(owner, state)
        except Exception as exc:
            print(f">>> ADD_TRACE ERROR: {type(exc).__name__}: {exc}")
            raise
        print(">>> ADD_TRACE RESULT: "
              f"status={getattr(result.status, 'value', result.status)}, "
              f"block={result.block_height}, batch={result.batch_id}")
        return result

    async def logging_build_trace(
        owner: Any,
        state: weil_tracer.TraceState,
        event: str,
    ) -> Any:
        print(">>> TRACE: calling build_trace")
        print(json.dumps({"trace_id": state.trace_id, "event": event}, indent=2))
        return await original_build_trace(owner, state, event)

    async def logging_get_trace(owner: Any, state: weil_tracer.TraceState) -> Any:
        print(">>> TRACE: calling get_trace")
        trace = await original_get_trace(owner, state)
        print(json.dumps(trace, indent=2))
        return trace

    async def logging_publish_trace(owner: Any, trace: Any) -> Any:
        print(">>> TRACE: publishing to topic audits")
        print(json.dumps(trace, indent=2))
        try:
            result = await original_publish_trace(owner, trace)
        except Exception as exc:
            print(f">>> PUBLISH ERROR: {type(exc).__name__}: {exc}")
            raise
        print(">>> PUBLISH RESULT: "
              f"status={getattr(result.status, 'value', result.status)}, "
              f"block={result.block_height}, batch={result.batch_id}")
        return result

    weil_tracer.initialize_trace = logging_initialize_trace
    weil_tracer.build_trace = logging_build_trace
    weil_tracer.get_trace = logging_get_trace
    weil_tracer.publish_trace = logging_publish_trace


def create_store_agent(identity: str):
    @weil_ai.agent(identity=identity, verify=VERIFY_SENTINEL_TLS)
    def factory():
        api_key = OPENAI_API_KEY or os.environ.get("OPENAI_API_KEY", "")
        if not api_key:
            raise RuntimeError("Fill OPENAI_API_KEY in this file or set it in your shell.")

        model = ChatOpenAI(
            model=MODEL,
            temperature=0,
            api_key=api_key,
        )

        return weil_langchain.create_agent(
            model=model,
            tools=[get_store_details, get_items, get_item_price],
            system_prompt=(
                "You are a store inventory agent. For inventory questions, always "
                "call get_store_details first, then get_items with the returned "
                "user_id, then get_item_price for at least one item before answering."
            ),
        )

    return factory()


def run_once(identity: str) -> None:
    print(f"\n=== Running with Weil identity: {identity} ===")
    agent = create_store_agent(identity)
    prompt = (
        "Get me all the items in the store. Also include the price of the coffee "
        "beans. Use the available tools to check instead of guessing."
    )
    result = agent.invoke({"messages": [{"role": "user", "content": prompt}]})
    print("\n>>> FINAL ANSWER")
    print(result["messages"][-1].content)


if __name__ == "__main__":
    install_trace_logger()
    for identity in WEIL_IDENTITIES:
        run_once(identity)
