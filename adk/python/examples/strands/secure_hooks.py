"""A Strands agent with a Weil identity: audited, traced, with a @secured tool.

Fill in the settings below, or export them, then run from python/:

    python examples/strands/secure_hooks.py
"""

import asyncio
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

import weil_ai
import weil_ai.strands as weil_strands
from strands import tool
from strands.models.openai import OpenAIModel


# Settings: each is taken from the environment variable of the same name when
# it is set, and otherwise from the value written here. Do not commit real keys.
def setting(name: str, value: str) -> str:
    return os.environ.get(name) or value


WEIL_AGENT_IDENTITY = setting("WEIL_AGENT_IDENTITY", "")  # the agent's Weil API key
OPENAI_API_KEY = setting("OPENAI_API_KEY", "")
OPENAI_MODEL = setting("OPENAI_MODEL", "gpt-5")  # or "gpt-5-mini", "gpt-4o"
WEIL_SERVICE = setting("WEIL_SERVICE", "engg.acme")
# The Sentinel the agent talks to; its TLS certificate is not verified.
SENTINEL_HOST = setting("SENTINEL_HOST", "https://sentinel.weilliptic.ai")

for name in ("WEIL_AGENT_IDENTITY", "OPENAI_API_KEY"):
    if not globals()[name]:
        sys.exit(f"{name} is not set: export it, or write it in {Path(__file__).name}")


@tool
def add(a: int, b: int) -> int:
    """Add two numbers."""
    print(">>> TOOL: add")
    return a + b


@tool
@weil_ai.secured_user(WEIL_SERVICE)
def multiply(a: int, b: int) -> int:
    """Multiply two numbers. Only wallets permitted on the service may call it."""
    print(">>> TOOL: multiply")
    return a * b


@weil_ai.agent(identity=WEIL_AGENT_IDENTITY, sentinel_host=SENTINEL_HOST, verify=False)
def create_math_agent():
    return weil_strands.create_agent(
        name="math_agent",
        model=OpenAIModel(
            client_args={"api_key": OPENAI_API_KEY},
            model_id=OPENAI_MODEL,
            # GPT-5 and the o-series reject "max_tokens"; this covers their
            # hidden reasoning too, so keep it generous.
            params={"max_completion_tokens": 4096},
        ),
        tools=[add, multiply],
        system_prompt=(
            "You are a math agent. Use your tools for arithmetic, and use your "
            "memory tools to save what you were asked and what you answered."
        ),
    )


async def main() -> None:
    agent = create_math_agent()
    result = await agent.invoke_async("What is (2 + 3) * 4?")
    print(result)


if __name__ == "__main__":
    asyncio.run(main())
