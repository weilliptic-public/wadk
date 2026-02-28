#!/usr/bin/env python3
"""Example: mnemonic wallet with derived accounts; confirm both can execute contract methods.

Run from python/ directory:
  cd python && python examples/example_derived.py
Or: PYTHONPATH=python python examples/example_derived.py
"""

import asyncio
import os
import sys
from pathlib import Path

# Allow importing weil_wallet when run as script
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from weil_wallet import (
    ContractId,
    MnemonicWallet,
    WeilClient,
    create_wallet,
)

# Optional: set SENTINEL_HOST env to override default
CONTRACT_ID_STR = "aaaaaawgpvetzeizoblifgn67vx5jqembzhu7woqtogtcrv6ceqwohfxpq"
METHOD_NAME = "balance_for"
METHOD_ARGS = '{"addr":"bdffb99d7949d570411e136c5f5623abe1733a9dd4da4dfbb0d67e1e1ac90fdd"}'

# Fixed mnemonic for reproducible test (or set MNEMONIC env, or leave None to generate)
MNEMONIC = os.environ.get(
    "MNEMONIC",
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
)


async def main() -> None:
    # Create wallet from mnemonic (or generate new)
    if MNEMONIC:
        wallet = create_wallet(mnemonic=MNEMONIC, generate_mnemonic=False)
        print("Using provided mnemonic")
    else:
        wallet = create_wallet(generate_mnemonic=True)
        print("Generated new mnemonic:", wallet.mnemonic[:60] + "...")

    # Derive two accounts
    acc0 = wallet.derive_account(0)
    acc1 = wallet.derive_account(1)
    print("Account 0 address:", acc0.address)
    print("Account 1 address:", acc1.address)

    contract_id = ContractId(CONTRACT_ID_STR)
    sentinel = os.environ.get("SENTINEL_HOST")

    # Execute balance_for with account 0
    print(f"\n--- Executing {METHOD_NAME} with account 0 ---")
    weil_wallet_0 = acc0.to_weil_wallet()
    async with WeilClient(weil_wallet_0, sentinel_host=sentinel) as client:
        result0 = await client.execute(contract_id, METHOD_NAME, METHOD_ARGS)
        print("Account 0 result:", result0.status)
        print("  txn_result:", result0.txn_result)

    # Execute balance_for with account 1
    print(f"\n--- Executing {METHOD_NAME} with account 1 ---")
    weil_wallet_1 = acc1.to_weil_wallet()
    async with WeilClient(weil_wallet_1, sentinel_host=sentinel) as client:
        result1 = await client.execute(contract_id, METHOD_NAME, METHOD_ARGS)
        print("Account 1 result:", result1.status)
        print("  txn_result:", result1.txn_result)

    print("\nDone. Both derived accounts can execute contract methods.")


if __name__ == "__main__":
    asyncio.run(main())
