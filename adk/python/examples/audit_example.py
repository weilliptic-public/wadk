#!/usr/bin/env python3
import asyncio
import os
import sys
from pathlib import Path

# Allow importing weil_wallet when run as script (e.g. python examples/example.py)
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from weil_wallet import PrivateKey, Wallet, WeilClient


async def main() -> None:
    # Path to private key file: script dir, cwd, or parent of script dir (e.g. python/)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    key_path = None

    for candidate in (
        os.path.join(script_dir, "private_key.wc"),
        "private_key.wc",
        os.path.join(os.path.dirname(script_dir), "private_key.wc"),
    ):
        if os.path.isfile(candidate):
            key_path = candidate
            break
    if key_path is None:
        raise FileNotFoundError(
            "private_key.wc not found. Place it in examples/, python/, or cwd."
        )

    pk = PrivateKey.from_file(key_path)
    wallet = Wallet(pk)

    print("Wallet initialized from private_key.wc")

    async with WeilClient(wallet) as client:
        print(f"Executing audit log")

        result = await client.audit("World is make sense")

        print("Result:")
        print(f"  status:       {result.status}")
        print(f"  block_height: {result.block_height}")
        print(f"  batch_id:     {result.batch_id}")
        print(f"  tx_idx:       {result.tx_idx}")
        print(f"  txn_result:   {result.txn_result}")
        print(f"  creation_time: {result.creation_time}")


if __name__ == "__main__":
    asyncio.run(main())
