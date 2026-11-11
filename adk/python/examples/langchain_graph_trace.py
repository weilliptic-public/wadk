#!/usr/bin/env python3
"""LangChain + Weil graph trace example.

This example asks one LangChain agent to call two independent tools in the
same turn. If the model emits parallel tool calls, both tool nodes share the
same parent. If a tool delegates to another Weil-wrapped agent, child events
attach under that tool node because the middleware reserves the tool node id
before running the handler.
"""

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


MODEL = os.environ.get("OPENAI_MODEL", "gpt-5.1")
IDENTITY = "weil_xxx"
OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")


@tool
def lookup_inventory(sku: str) -> str:
    """Return inventory for one SKU."""
    return json.dumps({"sku": sku, "available": 12, "warehouse": "SFO-1"})


@tool
def lookup_shipping(zipcode: str) -> str:
    """Return shipping options for one destination zipcode."""
    return json.dumps(
        {
            "zipcode": zipcode,
            "options": [
                {"service": "ground", "days": 3},
                {"service": "priority", "days": 1},
            ],
        }
    )


def install_graph_trace_logger() -> None:
    original_build_trace = weil_tracer.build_trace

    async def logging_build_trace(
        owner: Any,
        state: weil_tracer.TraceState,
        event: str,
    ) -> Any:
        node = json.loads(event)
        print(
            "TRACE NODE",
            json.dumps(
                {
                    "trace_id": state.trace_id,
                    "id": node["id"],
                    "parent_id": node["parent_id"],
                    "type": node["type"],
                    "content": node["content"],
                },
                indent=2,
            ),
        )
        return await original_build_trace(owner, state, event)

    weil_tracer.build_trace = logging_build_trace


@weil_ai.agent(identity=IDENTITY, verify=VERIFY_SENTINEL_TLS)
def create_order_agent():
    model = ChatOpenAI(
        model=MODEL,
        temperature=0,
        api_key=OPENAI_API_KEY,
    )
    return weil_langchain.create_agent(
        model=model,
        tools=[lookup_inventory, lookup_shipping],
        system_prompt=(
            "You are an order readiness agent. For readiness questions, call "
            "lookup_inventory and lookup_shipping in the same turn when both "
            "facts are needed, then answer from the tool results."
        ),
    )


if __name__ == "__main__":
    install_graph_trace_logger()
    agent = create_order_agent()
    result = agent.invoke(
        {
            "messages": [
                {
                    "role": "user",
                    "content": (
                        "Can sku_coffee ship to 94105 tomorrow? Check stock and "
                        "shipping options before answering."
                    ),
                }
            ]
        }
    )
    print(result["messages"][-1].content)
