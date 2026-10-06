#!/usr/bin/env python3
"""Use a Linux secure-memory wallet with WeilAgent.

The full wallet file is parsed inside protected native memory. Python receives
only the selected account's public key, address, and organization metadata.

    chmod 600 wallet.wc
    WEIL_PRIVATE_KEY_FILE=wallet.wc python examples/secure_agent.py
"""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import weil_ai


class MyAgent:
    def run(self, query: str) -> str:
        return f"handled: {query}"


key_path = Path(os.environ.get("WEIL_PRIVATE_KEY_FILE", "wallet.wc"))
# The decorator resolves the full wallet in native secure memory. Process
# hardening happens before the file is opened or parsed.
@weil_ai.agent(
    key_path,
    secure_wallet=True,
    require_lock=True,
    harden_process=True,
)
def create_agent() -> MyAgent:
    return MyAgent()


if __name__ == "__main__":
    agent = None
    try:
        agent = create_agent()
        print("secure memory:", agent.weil_wallet.security_status())
        print(agent.run("hello"))
        res = agent.audit("hello")
        print("auth headers:", agent.get_auth_headers())
    finally:
        if agent is not None:
            agent.weil_wallet.close()
