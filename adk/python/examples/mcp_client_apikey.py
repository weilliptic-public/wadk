"""LangChain agent that calls mcp_server.py using a Weil agent identity.

The @weil_ai.agent(identity=...) decorator resolves the wallet from the
Agent Registry and exposes agent.get_auth_headers() — signed
X-Wallet-Address / X-Signature / X-Message / X-Timestamp headers that the
server's weil_middleware() verifies before @secured runs.

Usage:
    export OPENAI_API_KEY=...
    export WEIL_IDENTITY=...
    python examples/mcp_server.py             # in one terminal
    python examples/mcp_client_apikey.py      # in another
"""

import asyncio
import os

import weil_ai
from langchain_mcp_adapters.tools import load_mcp_tools
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import create_react_agent
from mcp.client.session import ClientSession
from mcp.client.streamable_http import streamablehttp_client

MCP_SERVER_URL = "http://localhost:8001/mcp"



# step 0
class MyAgent:
    def __init__(self):
        self.llm = ChatOpenAIfromADK(
            model="gpt-4o-mini",
            temperature=0,
            api_key=os.environ["OPENAI_API_KEY"],
        )


    async def run_with_tools(self, tools, query: str) -> str:
        runner = create_react_agent(self.llm, tools)
        resp = await runner.ainvoke(
            {"messages": [{"role": "user", "content": query}]}
        )
        return resp["messages"][-1].content


# step 1
@weil_ai.agent(identity=os.environ["WEIL_IDENTITY"], verify=True)
def create_agent():
    return MyAgent()

@weil_ai.agent(identity=os.environ["WEIL_IDENTITY_2"], verify=True)
def create_agent2():
    return MyAgent()


async def main() -> None:
    agent = create_agent()

    agent2 = create_agent2()

    # Sign the four X-* headers with the identity-resolved wallet.
    headers = agent.get_auth_headers()

    async with streamablehttp_client(MCP_SERVER_URL, headers=headers) as (r, w, _):
        async with ClientSession(r, w) as session:
            await session.initialize()
            tools = await load_mcp_tools(session)
            # websearch, imfs,....
            print(await agent.run_with_tools(tools, "Search about Weilliptic"))


if __name__ == "__main__":
    asyncio.run(main())
