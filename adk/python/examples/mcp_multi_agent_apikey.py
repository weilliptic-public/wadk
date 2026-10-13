"""Multiple identity agents calling a secured MCP tool.

Run ``examples/mcp_server.py`` separately, then provide identities for agents
that should both pass and fail ``@secured("engg.weil")``.
"""

from __future__ import annotations

import asyncio
import os

import weil_ai
from langchain_mcp_adapters.tools import load_mcp_tools
from mcp.client.session import ClientSession
from mcp.client.streamable_http import streamablehttp_client

MCP_SERVER_URL = os.environ.get("MCP_SERVER_URL", "http://localhost:8001/mcp")


class ToolAgent:
    async def run_with_tools(self, tools, query: str) -> str:
        tool = next(t for t in tools if t.name == "search")
        return await tool.ainvoke({"query": query})


def _creds_from_env() -> dict | None:
    access_key = os.environ.get("WEIL_S3_ACCESS_KEY") or os.environ.get(
        "AWS_ACCESS_KEY_ID"
    )
    secret_key = os.environ.get("WEIL_S3_SECRET_KEY") or os.environ.get(
        "AWS_SECRET_ACCESS_KEY"
    )
    if not access_key or not secret_key:
        return None
    creds = {"access_key": access_key, "secret_key": secret_key}
    region = os.environ.get("WEIL_S3_REGION") or os.environ.get("AWS_REGION")
    bucket = os.environ.get("WEIL_S3_BUCKET") or os.environ.get("AWS_BUCKET_NAME")
    if region:
        creds["region"] = region
    if bucket:
        creds["bucket"] = bucket
    return creds


def create_agent(identity: str, credentials: dict | None = None):
    @weil_ai.agent(identity=identity, credentials=credentials, verify=True)
    def factory():
        return ToolAgent()

    return factory()


async def call_secured_tool(label: str, agent) -> None:
    headers = agent.get_auth_headers()
    async with streamablehttp_client(MCP_SERVER_URL, headers=headers) as (r, w, _):
        async with ClientSession(r, w) as session:
            await session.initialize()
            tools = await load_mcp_tools(session)
            result = await agent.run_with_tools(tools, "Search about Weilliptic")
            print(f"{label}: {result}")


async def main() -> None:
    credentials = _creds_from_env()
    identities = {
        "allowed": os.environ["WEIL_IDENTITY_ALLOWED"],
        "denied": os.environ["WEIL_IDENTITY_DENIED"],
    }
    agents = {
        label: create_agent(identity, credentials=credentials)
        for label, identity in identities.items()
    }
    await asyncio.gather(
        *(call_secured_tool(label, agent) for label, agent in agents.items())
    )


if __name__ == "__main__":
    asyncio.run(main())
