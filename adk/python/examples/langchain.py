#!/usr/bin/env python3
"""Example: LangChain agent with Weil identity and audit.

- @weil_agent on your agent factory; create_agent(wallet_path="...") to pass the key
- agent.get_auth_headers() to get signed MCP headers
- agent.audit(log) to log on-chain
"""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import weil_ai
from langchain_core.messages import HumanMessage
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI

_KEY = Path(__file__).resolve().parent.parent / "private_key.wc"


@tool
def calculator(expr: str) -> str:
    """Evaluate a math expression. Input should be a valid Python expression."""
    return str(eval(expr))


class MyAgent:
    def __init__(self):
        self.llm = ChatOpenAI(
            model="gpt-4o-mini",
            temperature=0,
            api_key=os.environ["OPENAI_API_KEY"],
        )
        self.tools = [calculator]
        self.llm_with_tools = self.llm.bind_tools(self.tools)

    def run(self, query: str) -> str:
        messages = [HumanMessage(content=query)]
        response = self.llm_with_tools.invoke(messages)

        if response.tool_calls:
            for tc in response.tool_calls:
                tool_map = {t.name: t for t in self.tools}
                if tc["name"] in tool_map:
                    result = tool_map[tc["name"]].invoke(tc["args"])
                    return result
        return response.content or ""


@weil_ai.agent(_KEY)
def create_agent():
    return MyAgent()


if __name__ == "__main__":
    agent = create_agent()

    query = "What is the square root of 16 ?"
    response = agent.run(query)
    print(response)

    log = json.dumps({"method": "run", "query": query})
    agent.audit(log)
