#!/usr/bin/env python3
"""Run random single-tool authorization checks through secured FastMCP tools.

Start ``python examples/agents_fleet/mcp_server.py`` separately, then run this
script with fleet_config.py and OPENAI_API_KEY configured.
"""

import asyncio
import os
import random
import sys
from pathlib import Path

from dotenv import load_dotenv

load_dotenv(Path(__file__).with_name(".env"), override=True)

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1].parent))

import weil_ai
from langchain_core.messages import HumanMessage
from langchain_mcp_adapters.tools import load_mcp_tools
from langchain_openai import ChatOpenAI
from mcp.client.session import ClientSession
from mcp.client.streamable_http import streamablehttp_client

from fleet_config import WEIL_IDENTITIES

PROMPTS_PATH = Path(__file__).with_name("prompts.md")
MCP_SERVER_URL = os.environ.get("MCP_SERVER_URL", "http://127.0.0.1:8001/mcp")
DRAWS_PER_RUN = 500
MAX_PARALLEL_AGENTS = 25


def load_prompts(path: Path = PROMPTS_PATH) -> list[str]:
    prompts = [
        line.removeprefix("- ").strip()
        for line in path.read_text().splitlines()
        if line.strip()
    ]
    if not prompts:
        raise ValueError(f"No prompts found in {path}.")
    return prompts


class FleetAgent:
    def __init__(self) -> None:
        self.llm = ChatOpenAI(
            model="gpt-4o-mini",
            temperature=0.3,
            api_key=os.environ["OPENAI_API_KEY"],
        )

    async def run(self, identity: str, prompt: str) -> tuple[str, str, str]:
        @weil_ai.agent(identity=identity)
        def create_agent() -> FleetAgent:
            return self

        agent = create_agent()
        try:
            async with streamablehttp_client(
                MCP_SERVER_URL,
                headers=agent.get_auth_headers(),
            ) as (read_stream, write_stream, _):
                async with ClientSession(read_stream, write_stream) as session:
                    await session.initialize()
                    tools = await load_mcp_tools(session)
                    if not tools:
                        raise RuntimeError("The MCP server exposes no tools.")

                    response = await self.llm.bind_tools(
                        tools,
                        parallel_tool_calls=False,
                    ).ainvoke(
                        [
                            HumanMessage(
                                content=(
                                    "Select exactly one tool call for this request. "
                                    "Never call more than one tool.\n" + prompt
                                )
                            )
                        ]
                    )
                    if len(response.tool_calls) != 1:
                        return (
                            "not called",
                            "invalid",
                            f"Expected exactly one tool call, received {len(response.tool_calls)}.",
                        )
                    tool_call = response.tool_calls[0]
                    tool_name = tool_call["name"]
                    try:
                        result = await session.call_tool(
                            tool_name,
                            tool_call["args"],
                        )
                    except Exception as error:
                        return tool_name, "error", str(error)
                    text = "\n".join(
                        item.text
                        for item in result.content
                        if hasattr(item, "text")
                    )
                    rejected = result.isError or "does not have permission" in text
                    return (
                        tool_name,
                        "rejected" if rejected else "executed",
                        text,
                    )
        finally:
            await agent.close()


async def main() -> None:
    if len(WEIL_IDENTITIES) < 2:
        raise SystemExit(
            "Add at least two identities to examples/agents_fleet/fleet_config.py."
        )
    if not os.environ.get("OPENAI_API_KEY"):
        raise SystemExit("Set OPENAI_API_KEY to run the live model check.")

    prompts = load_prompts()
    jobs = asyncio.Queue()
    for job_id in range(DRAWS_PER_RUN):
        jobs.put_nowait(
            (job_id, random.choice(WEIL_IDENTITIES), random.choice(prompts))
        )

    async def worker() -> None:
        while True:
            try:
                job_id, identity, prompt = jobs.get_nowait()
            except asyncio.QueueEmpty:
                return
            try:
                tool_name, outcome, response = await FleetAgent().run(identity, prompt)
            except Exception as error:
                tool_name, outcome, response = "not called", "error", str(error)
            print(
                f"Job {job_id + 1}: Tool: {tool_name} | Outcome: {outcome} | Prompt: {prompt}"
            )
            print(f"Response: {response}")
            jobs.task_done()

    await asyncio.gather(
        *(worker() for _ in range(min(MAX_PARALLEL_AGENTS, DRAWS_PER_RUN)))
    )


if __name__ == "__main__":
    asyncio.run(main())
