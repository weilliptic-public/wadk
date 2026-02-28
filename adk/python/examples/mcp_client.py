"""LangChain agent that calls the mcp_server.py MCP server.

Auth headers are signed with the wallet's private key so the server-side
weil_middleware() can verify ownership before any tool is called.

Usage:
    # start the server first
    python examples/mcp_server.py

    # then run this client
    python examples/mcp_client.py
"""

import asyncio
import os
from langchain_mcp_adapters.tools import load_mcp_tools
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import create_react_agent
from mcp.client.session import ClientSession
from mcp.client.streamable_http import streamablehttp_client
from weil_wallet import PrivateKey, Wallet, WeilClient
from weil_ai.auth import build_auth_headers

MCP_SERVER_URL = "http://localhost:8001/mcp"


def create_weil_client() -> tuple[Wallet, WeilClient]:
    """Load private key from .wc file and return the Wallet alongside a WeilClient."""
    script_dir = os.path.dirname(os.path.abspath(__file__))

    for candidate in (
        os.path.join(script_dir, "private_key.wc"),
        "private_key.wc",
        os.path.join(os.path.dirname(script_dir), "private_key.wc"),
    ):
        if os.path.isfile(candidate):
            pk = PrivateKey.from_file(candidate)
            wallet = Wallet(pk)

            return WeilClient(wallet)
    raise FileNotFoundError(
        "private_key.wc not found. Place it in examples/, python/, or cwd."
    )


client = create_weil_client()


async def run(query: str) -> None:
    headers = build_auth_headers(client._wallet)

    async with streamablehttp_client(MCP_SERVER_URL, headers=headers) as (
        read,
        write,
        _,
    ):
        async with ClientSession(read, write) as session:
            await session.initialize()

            tools = await load_mcp_tools(session)
            llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0,
                api_key=os.environ["OPENAI_API_KEY"],
            )
            agent = create_react_agent(llm, tools)

            response = await agent.ainvoke(
                {"messages": [{"role": "user", "content": query}]}
            )
            print(response["messages"][-1].content)


if __name__ == "__main__":
    asyncio.run(run("Search about Techox!"))
