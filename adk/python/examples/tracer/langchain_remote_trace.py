#!/usr/bin/env python3
"""LangChain planner calling a remote Weil agent while sharing one trace.

Run:
    python examples/tracer/langchain_remote_trace.py

Fill in the constants below first. The script starts Agent B as a local HTTP
service, then runs Agent A. Agent A performs local LangChain tool work, calls
Agent B remotely, then completes the answer. The trace logger prints every
``add_trace`` and ``build_trace`` call so you can verify both agents append to
the same trace id.
"""

from __future__ import annotations

import json
import os
import socket
import sys
import threading
import time
from pathlib import Path
from typing import Any

EXAMPLE_DIR = Path(__file__).resolve().parent
REPO_ROOT = EXAMPLE_DIR.parents[1]
sys.path = [path for path in sys.path if Path(path or ".").resolve() != EXAMPLE_DIR]
sys.path.insert(0, str(REPO_ROOT))

import uvicorn
import weil_ai
import weil_ai.langchain as weil_langchain
import weil_ai.tracer as weil_tracer
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI


OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
OPENAI_MODEL = "gpt-5.1"

PLANNER_WEIL_IDENTITY = "weil_xx"
FULFILLMENT_WEIL_IDENTITY = "weil_xxx"
VERIFY_SENTINEL_TLS = False

FULFILLMENT_HOST = "127.0.0.1"
FULFILLMENT_PORT = 8123

REMOTE_FULFILLMENT: Any = None


def _require_filled_constants() -> None:
    missing = [
        name
        for name, value in {
            "OPENAI_API_KEY": OPENAI_API_KEY,
            "PLANNER_WEIL_IDENTITY": PLANNER_WEIL_IDENTITY,
            "FULFILLMENT_WEIL_IDENTITY": FULFILLMENT_WEIL_IDENTITY,
        }.items()
        if not value or "fill" in value
    ]
    if missing:
        raise RuntimeError(f"Fill these constants at the top of the file: {missing}")


class FulfillmentAgent:
    """Remote Agent B: deterministic work so trace shape is easy to inspect."""

    def check_fulfillment(self, order_json: str) -> str:
        order = json.loads(order_json)
        sku = order["sku"]
        destination = order["destination_zip"]

        weil_tracer.record_tool_call(
            "fulfillment.parse_order",
            {"order_json": order_json},
            {"sku": sku, "destination_zip": destination},
        )
        weil_tracer.record_tool_call(
            "fulfillment.reserve_inventory",
            {"sku": sku, "quantity": order["quantity"]},
            {"reservation_id": "resv-coffee-001", "reserved": True},
        )
        weil_tracer.record_tool_call(
            "fulfillment.rate_shipper",
            {"destination_zip": destination, "service": "priority"},
            {"carrier": "WeilShip", "days": 1, "cost": 7.25},
        )
        return json.dumps(
            {
                "reservation_id": "resv-coffee-001",
                "reserved": True,
                "carrier": "WeilShip",
                "ship_days": 1,
                "shipping_cost": 7.25,
            }
        )


@tool
def normalize_order(user_request: str) -> str:
    """Extract the order fields before calling the remote fulfillment agent."""
    return json.dumps(
        {
            "sku": "sku_coffee",
            "quantity": 2,
            "destination_zip": "94105",
            "requested_arrival": "tomorrow",
            "source_request": user_request,
        }
    )


@tool
def calculate_local_tax(order_json: str) -> str:
    """Calculate local tax for the order before remote fulfillment."""
    order = json.loads(order_json)
    subtotal = 14.00 * int(order["quantity"])
    return json.dumps(
        {
            "sku": order["sku"],
            "quantity": order["quantity"],
            "subtotal": subtotal,
            "tax": round(subtotal * 0.085, 2),
        }
    )


@tool
def ask_fulfillment_agent(order_json: str) -> str:
    """Ask the remote fulfillment agent to reserve stock and rate shipping."""
    if REMOTE_FULFILLMENT is None:
        raise RuntimeError("Remote fulfillment agent has not been initialized")
    return REMOTE_FULFILLMENT.check_fulfillment(order_json)


def install_trace_logger() -> None:
    original_initialize_trace = weil_tracer.initialize_trace
    original_build_trace = weil_tracer.build_trace
    original_get_trace = weil_tracer.get_trace
    original_publish_trace = weil_tracer.publish_trace

    async def logging_initialize_trace(owner: Any, state: weil_tracer.TraceState) -> Any:
        print("\nTRACE add_trace")
        print(json.dumps({"trace_id": state.trace_id, "root": state.root}, indent=2))
        return await original_initialize_trace(owner, state)

    async def logging_build_trace(owner: Any, state: weil_tracer.TraceState, event: str) -> Any:
        node = json.loads(event)
        print("TRACE build_trace")
        print(
            json.dumps(
                {
                    "trace_id": state.trace_id,
                    "id": node["id"],
                    "parent_id": node["parent_id"],
                    "type": node["type"],
                    "content": node["content"],
                },
                indent=2,
            )
        )
        return await original_build_trace(owner, state, event)

    async def logging_get_trace(owner: Any, state: weil_tracer.TraceState) -> Any:
        print("TRACE get_trace")
        return await original_get_trace(owner, state)

    async def logging_publish_trace(owner: Any, trace: Any) -> Any:
        print("TRACE publish_trace")
        print(json.dumps(trace, indent=2, default=repr))
        return await original_publish_trace(owner, trace)

    weil_tracer.initialize_trace = logging_initialize_trace
    weil_tracer.build_trace = logging_build_trace
    weil_tracer.get_trace = logging_get_trace
    weil_tracer.publish_trace = logging_publish_trace


def create_fulfillment_agent() -> Any:
    @weil_ai.agent(identity=FULFILLMENT_WEIL_IDENTITY, verify=VERIFY_SENTINEL_TLS)
    def factory() -> FulfillmentAgent:
        return FulfillmentAgent()

    return factory()


def create_planner_agent() -> Any:
    @weil_ai.agent(identity=PLANNER_WEIL_IDENTITY, verify=VERIFY_SENTINEL_TLS)
    def factory() -> Any:
        model = ChatOpenAI(
            model=OPENAI_MODEL,
            temperature=0,
            api_key=OPENAI_API_KEY,
        )
        return weil_langchain.create_agent(
            model=model,
            tools=[normalize_order, calculate_local_tax, ask_fulfillment_agent],
            system_prompt=(
                "You are Agent A, an order planner. For every order request, do "
                "these steps in order: call normalize_order, call calculate_local_tax "
                "with the normalized order JSON, call ask_fulfillment_agent with the "
                "same normalized order JSON, then answer from the tool results. "
                "Do not skip the remote fulfillment agent."
            ),
        )

    return factory()


def start_fulfillment_server(agent: Any) -> uvicorn.Server:
    app = agent.to_secure_http()
    config = uvicorn.Config(
        app,
        host=FULFILLMENT_HOST,
        port=FULFILLMENT_PORT,
        log_level="warning",
    )
    server = uvicorn.Server(config)
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()

    deadline = time.monotonic() + 10
    while not server.started:
        if time.monotonic() > deadline:
            raise RuntimeError("Fulfillment server did not start")
        time.sleep(0.05)
    return server


def assert_port_available() -> None:
    with socket.socket() as sock:
        try:
            sock.bind((FULFILLMENT_HOST, FULFILLMENT_PORT))
        except OSError as exc:
            raise RuntimeError(
                f"{FULFILLMENT_HOST}:{FULFILLMENT_PORT} is already in use; "
                "change FULFILLMENT_PORT at the top of this file."
            ) from exc


def main() -> None:
    global REMOTE_FULFILLMENT

    _require_filled_constants()
    assert_port_available()
    install_trace_logger()

    fulfillment = create_fulfillment_agent()
    server = start_fulfillment_server(fulfillment)
    try:
        planner = create_planner_agent()
        REMOTE_FULFILLMENT = planner.remote(
            f"http://{FULFILLMENT_HOST}:{FULFILLMENT_PORT}"
        )
        prompt = (
            "Plan an order for two bags of sku_coffee to 94105 for arrival "
            "tomorrow. Do the local planning first, then use the fulfillment "
            "agent for reservation and shipping."
        )
        result = planner.invoke({"messages": [{"role": "user", "content": prompt}]})
        print("\nFINAL ANSWER")
        print(result["messages"][-1].content)
    finally:
        server.should_exit = True


if __name__ == "__main__":
    main()
