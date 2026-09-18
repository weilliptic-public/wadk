#!/usr/bin/env python3
"""LangChain agent with a Weil identity, using an API key."""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import weil_ai
from langchain_core.messages import HumanMessage
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI


@tool
def calculator(expression: str) -> str:
    """Evaluate a simple arithmetic expression."""
    allowed = {
        "+": lambda a, b: a + b,
        "-": lambda a, b: a - b,
        "*": lambda a, b: a * b,
        "/": lambda a, b: a / b,
        "**": lambda a, b: a ** b,
    }

    parts = expression.strip().split()
    if len(parts) == 3:
        left, op, right = parts
        try:
            a, b = float(left), float(right)
        except ValueError:
            return "Numbers must be numeric."
        if op not in allowed:
            return f"Unsupported operator: {op}"
        return str(allowed[op](a, b))

    if expression.strip().lower().startswith("sqrt"):
        value = expression.strip()[4:].strip("()")
        try:
            num = float(value)
        except ValueError:
            return "sqrt needs a number."
        if num < 0:
            return "Cannot take the square root of a negative number."
        return str(num ** 0.5)

    return "Use the form: <number> <operator> <number>  or  sqrt(<number>)"


class MyAgent:
    def __init__(self):
        self.llm = ChatOpenAI(
            model="gpt-4o-mini",
            temperature=0,
            api_key=os.environ["OPENAI_API_KEY"],
        )
        self.tools = [calculator]
        self.tool_map = {tool.name: tool for tool in self.tools}
        self.llm_with_tools = self.llm.bind_tools(self.tools)

    def run(self, query: str) -> str:
        response = self.llm_with_tools.invoke([HumanMessage(content=query)])

        if response.tool_calls:
            for call in response.tool_calls:
                if call["name"] in self.tool_map:
                    return self.tool_map[call["name"]].invoke(call["args"])

        return response.content or ""

# Optional: Put your S3 Credentials here
creds = {
  "access_key_id": os.environ["AWS_ACCESS_KEY_ID"],
  "secret_access_key": os.environ["AWS_SECRET_ACCESS_KEY"],
  "region": os.environ["AWS_REGION"],
  "bucket_name": os.environ["AWS_S3_BUCKET"],
}

@weil_ai.agent(api_key=os.environ["WEIL_AGENT_API_KEY"], credentials=creds, verify=True)
def create_agent():
    return MyAgent()

if __name__ == "__main__":
    agent = create_agent()

    query = "What is the square root of 16?"
    print(agent.run(query))

    log = json.dumps({"method": "run", "query": query})
    result = agent.audit(log)
    print(
        f"Audit: status={result.status.value}, "
        f"block={result.block_height}, batch={result.batch_id}"
    )
