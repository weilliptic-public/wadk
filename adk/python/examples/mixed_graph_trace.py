#!/usr/bin/env python3
"""LangChain multi-agent graph trace example.

This example uses real LangChain agents:

- a supervisor agent receives the user prompt
- the supervisor is prompted to call two branch tools in the same turn
- each branch tool invokes a child LangChain agent
- each child agent performs serial tool calls

Expected graph shape:

root prompt
├── inventory_branch                  parent_id: null
│   └── inventory child subtrace       parent_id: inventory_branch
│       ├── lookup_inventory           parent_id: inventory child subtrace
│       └── lookup_supplier_eta        parent_id: lookup_inventory
└── shipping_branch                    parent_id: null
    └── shipping child subtrace        parent_id: shipping_branch
        ├── lookup_shipping            parent_id: shipping child subtrace
        └── lookup_shipping_cutoff     parent_id: lookup_shipping
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


MODEL = "gpt-5.1"
IDENTITY = "weil_xxx"
OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
VERIFY_SENTINEL_TLS = False


@tool
def lookup_inventory(sku: str) -> str:
    """Return current stock for one SKU."""
    return json.dumps(
        {
            "sku": sku,
            "available": 12,
            "warehouse": "SFO-1",
            "inventory_receipt": "inv-receipt-sku-coffee-sfo-1",
        }
    )


@tool
def lookup_supplier_eta(sku: str, inventory_receipt: str) -> str:
    """Return ETA using the inventory_receipt returned by lookup_inventory."""
    if inventory_receipt != "inv-receipt-sku-coffee-sfo-1":
        return json.dumps(
            {
                "error": "invalid_inventory_receipt",
                "hint": "Call lookup_inventory first and pass its inventory_receipt.",
            }
        )
    return json.dumps(
        {
            "sku": sku,
            "inventory_receipt": inventory_receipt,
            "eta_days": 0,
            "source": "warehouse_stock",
        }
    )


@tool
def lookup_shipping(zipcode: str) -> str:
    """Return shipping options for one destination zipcode."""
    return json.dumps(
        {
            "zipcode": zipcode,
            "shipping_receipt": "ship-receipt-94105-priority",
            "options": [
                {"service": "ground", "days": 3},
                {"service": "priority", "days": 1},
            ],
        }
    )


@tool
def lookup_shipping_cutoff(zipcode: str, shipping_receipt: str) -> str:
    """Return cutoff using the shipping_receipt returned by lookup_shipping."""
    if shipping_receipt != "ship-receipt-94105-priority":
        return json.dumps(
            {
                "error": "invalid_shipping_receipt",
                "hint": "Call lookup_shipping first and pass its shipping_receipt.",
            }
        )
    return json.dumps(
        {
            "zipcode": zipcode,
            "shipping_receipt": shipping_receipt,
            "service": "priority",
            "cutoff_local": "17:00",
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
                    "contract_event": weil_tracer.contract_trace_event(event),
                },
                indent=2,
            ),
        )
        return await original_build_trace(owner, state, event)

    weil_tracer.build_trace = logging_build_trace


@weil_ai.agent(identity=IDENTITY, verify=VERIFY_SENTINEL_TLS)
def create_multi_agent():
    model = ChatOpenAI(
        model=MODEL,
        temperature=0,
        api_key=OPENAI_API_KEY,
    )

    inventory_agent = weil_langchain.create_agent(
        model=model,
        tools=[lookup_inventory, lookup_supplier_eta],
        system_prompt=(
            "You are the inventory subagent. For every request, first call "
            "lookup_inventory. Then call lookup_supplier_eta using the sku and "
            "inventory_receipt returned by lookup_inventory. These two calls are "
            "serial: do not call lookup_supplier_eta until lookup_inventory returns. "
            "Then summarize the result."
        ),
    )

    shipping_agent = weil_langchain.create_agent(
        model=model,
        tools=[lookup_shipping, lookup_shipping_cutoff],
        system_prompt=(
            "You are the shipping subagent. For every request, first call "
            "lookup_shipping. Then call lookup_shipping_cutoff using zipcode "
            "and the shipping_receipt returned by lookup_shipping. These two calls "
            "are serial: do not call lookup_shipping_cutoff until lookup_shipping "
            "returns. Then summarize the result."
        ),
    )

    @tool
    def inventory_branch(sku: str) -> str:
        """Ask the inventory subagent to check stock and stock ETA for one SKU."""
        result = inventory_agent.invoke(
            {
                "messages": [
                    {
                        "role": "user",
                        "content": (
                            f"Check stock for {sku}. Use both inventory tools "
                            "in the required serial order."
                        ),
                    }
                ]
            }
        )
        return result["messages"][-1].content

    @tool
    def shipping_branch(zipcode: str) -> str:
        """Ask the shipping subagent to check shipping options and cutoff."""
        result = shipping_agent.invoke(
            {
                "messages": [
                    {
                        "role": "user",
                        "content": (
                            f"Check shipping to {zipcode}. Use both shipping "
                            "tools in the required serial order."
                        ),
                    }
                ]
            }
        )
        return result["messages"][-1].content

    return weil_langchain.create_agent(
        model=model,
        tools=[inventory_branch, shipping_branch],
        system_prompt=(
            "You are the supervisor. For order readiness questions, call both "
            "inventory_branch and shipping_branch in the same tool-calling turn "
            "before answering. These two branch calls are independent and should "
            "be issued together, not one after the other. Do not answer until "
            "both branch results are back."
        ),
    )


if __name__ == "__main__":
    install_graph_trace_logger()
    agent = create_multi_agent()
    result = agent.invoke(
        {
            "messages": [
                {
                    "role": "user",
                    "content": (
                        "Can sku_coffee ship to 94105 tomorrow? Check stock and "
                        "shipping options before answering. Use subagents."
                    ),
                }
            ]
        }
    )
    print("FINAL ANSWER")
    print(result["messages"][-1].content)
