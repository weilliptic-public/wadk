import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

import weil_ai
import weil_ai.langchain as weil_langchain
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI


AGENT_API_KEY = "<AGENT_IDENTITY>"
KEY="<api_key>"


@tool
@weil_ai.secured_user("engg.weil")
def add(a: int, b: int) -> int:
    """Add two numbers."""
    print(">>> TOOL: add")
    return a + b


@weil_ai.agent(identity=AGENT_API_KEY, verify=False)
def create_math_agent():
    model = ChatOpenAI(
        model="gpt-5.1",
        temperature=0,
        api_key=KEY,
    )

    return weil_langchain.create_agent(
        model=model,
        tools=[add],
        system_prompt="You are a math agent. Use tools for arithmetic always use your memory tools to save the information regarding asked and response.",
    )


if __name__ == "__main__":
    agent = create_math_agent()

    # This is optional when the agent was created through
    # weil_langchain.create_agent(...). It is useful as a fallback for already
    # created LangChain graphs.
    agent.setup_secure_langchain()

    result = agent.invoke(
        {
            "messages": [
                {
                    "role": "user",
                    "content": "what is my name",
                }
            ]
        }
    )
    print(result["messages"][-1].content)
