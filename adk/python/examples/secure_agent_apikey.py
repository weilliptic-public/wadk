#!/usr/bin/env python3
"""Resolve an Agent Registry wallet through native HTTPS and secure memory.

Required environment:
    WEIL_API_KEY

Optional external-storage environment:
    AWS_ACCESS_KEY_ID
    AWS_SECRET_ACCESS_KEY
    AWS_REGION
    AWS_BUCKET_NAME
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import weil_ai


class MyAgent:
    def run(self, query: str) -> str:
        return f"handled: {query}"


@weil_ai.agent(
    api_key=os.environ["WEIL_API_KEY"],
    sentinel_host=os.environ.get("SENTINEL_HOST", "https://sentinel.weilliptic.ai"),
    secure_wallet=True,
    require_lock=True,
    harden_process=True,
)
def create_agent() -> MyAgent:
    return MyAgent()


if __name__ == "__main__":
    agent = create_agent()
    try:
        print("secure memory:", agent.weil_wallet.security_status())
        print("organization:", agent.weil_wallet.org)
        print(agent.run("hello"))
        print("auth headers:", agent.get_auth_headers())
    finally:
        agent.weil_wallet.close()
