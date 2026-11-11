from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import uvicorn
from fastmcp import FastMCP
from langchain_core.messages import HumanMessage
from langchain_mcp_adapters.tools import load_mcp_tools
from langchain_openai import ChatOpenAI
from mcp.client.session import ClientSession
from mcp.client.streamable_http import streamablehttp_client

import weil_ai
from weil_ai.mcp import audit, secured, weil_middleware


MCP_SERVER_URL = "http://127.0.0.1:8001/mcp"
MCP_HOST = "127.0.0.1"
MCP_PORT = 8001
SCOPE = "engg.acme"
VERIFY_TLS = False
IDENTITY = "weil_xxx"
OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
MODEL = "gpt-4o-mini"



## server side
mcp = FastMCP("secured-audit-starter")


@mcp.tool(name="add_numbers")
@secured(SCOPE)
@audit()
async def add_numbers(a: int, b: int) -> str:
    return json.dumps({"a": a, "b": b, "sum": a + b})


@mcp.tool(name="shout")
@secured(SCOPE)
@audit()
async def shout(text: str) -> str:
    return text.upper()


app = mcp.http_app(transport="streamable-http")
app.add_middleware(weil_middleware())

def run_server() -> None:
    uvicorn.run(app, host=MCP_HOST, port=MCP_PORT)


## client side
class MyAgent:
    def __init__(self) -> None:
        self.llm = ChatOpenAI(
            model=MODEL,
            temperature=0,
            api_key=OPENAI_API_KEY,
        )

    async def run(self, session, query: str) -> str:
        tools = await load_mcp_tools(session)
        response = await self.llm.bind_tools(
            tools,
            parallel_tool_calls=False,
        ).ainvoke([HumanMessage(content=query)])

        if not response.tool_calls:
            return str(response.content or "")

        tool_call = response.tool_calls[0]
        tool_by_name = {tool.name: tool for tool in tools}
        tool = tool_by_name[tool_call["name"]]
        result = await tool.ainvoke(tool_call["args"])
        return str(result)


@weil_ai.agent(identity=IDENTITY, verify=VERIFY_TLS)
def build_agent():
    return MyAgent()


async def run_client() -> None:
    agent = build_agent()
    headers = await agent.aget_auth_headers()

    async with streamablehttp_client(MCP_SERVER_URL, headers=headers) as (
        read_stream,
        write_stream,
        _,
    ):
        async with ClientSession(read_stream, write_stream) as raw_session:
            session = agent.weil_session(raw_session)
            await session.initialize()

            for query in (
                "Use the add_numbers tool to add 2 and 3.",
                "Use the shout tool to uppercase hello from weil.",
            ):
                print(await agent.run(session, query))

    await agent.close()



def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("server", "client"))
    args = parser.parse_args()

    if args.mode == "server":
        run_server()
    else:
        asyncio.run(run_client())


if __name__ == "__main__":
    main()
