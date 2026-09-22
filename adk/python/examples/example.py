#!/usr/bin/env python3
"""Example: initialize wallet from private_key.wc and execute balance_for on a contract.

Run from python/ directory so weil_wallet is importable:
  cd python && python examples/example.py
Or with PYTHONPATH set:
  PYTHONPATH=python python examples/example.py
"""

import asyncio
import os
import sys
from pathlib import Path

# Allow importing weil_wallet when run as script (e.g. python examples/example.py)
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from weil_wallet import ContractId, Wallet, WeilClient


async def main() -> None:
    # Path to wallet export JSON file: script dir, cwd, or parent of script dir (e.g. python/)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    wallet_path = None
    for candidate in (
        os.path.join(script_dir, "wallet.wc"),
        "wallet.wc",
        os.path.join(os.path.dirname(script_dir), "wallet.wc"),
    ):
        if os.path.isfile(candidate):
            wallet_path = candidate
            break
    if wallet_path is None:
        raise FileNotFoundError(
            "wallet.wc not found. Place it in examples/, python/, or cwd."
        )

    wallet = Wallet.from_account_export_file(wallet_path)
    print("Wallet initialized from wallet.wc")

    contract_id = ContractId(
        "aaaaaayvitmkip5jdz524cnavebftb5prmgjv32eq5ppvpaxdwgu2knxmu"
    )
    method_name = "audit"
    method_args = '{"log": "Rust is awesome!"}'

    async with WeilClient(wallet) as client:
        print(f"Executing {method_name} on contract {contract_id}...")
        result = await client.execute(contract_id, method_name, method_args, True)
        print("Result:")
        print(f"  status:       {result.status}")
        print(f"  block_height: {result.block_height}")
        print(f"  batch_id:     {result.batch_id}")
        print(f"  tx_idx:       {result.tx_idx}")
        print(f"  txn_result:   {result.txn_result}")
        print(f"  creation_time: {result.creation_time}")


if __name__ == "__main__":
    print("Greetings from Weilliptic")
    asyncio.run(main())
