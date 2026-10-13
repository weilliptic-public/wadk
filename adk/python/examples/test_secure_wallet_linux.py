#!/usr/bin/env python3
"""Functional and core-dump test for the Linux secure wallet backend.

Loads the wallet securely (native secure memory) either from a wallet.wc file
or from the Agent Registry via an agent identity (masked wallet + remote signing).

Examples:
    python examples/test_secure_wallet_linux.py --wallet wallet.wc
    python examples/test_secure_wallet_linux.py --wallet wallet.wc --mode no-core
    python examples/test_secure_wallet_linux.py --wallet wallet.wc --mode inspect-core
    python examples/test_secure_wallet_linux.py --identity "$WEIL_IDENTITY"
    python examples/test_secure_wallet_linux.py --identity "$WEIL_IDENTITY" --mode no-core
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from weil_ai import WeilAgent
from weil_wallet import Wallet


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wallet", type=Path)
    parser.add_argument("--identity")
    parser.add_argument(
        "--include-creds",
        action="store_true",
        help="Attach AWS_* env credentials to the identity wallet fetch "
        "(for external wallets stored under those creds).",
    )
    parser.add_argument(
        "--sentinel",
        default="https://sentinel-local.weilliptic.ai",
    )
    parser.add_argument(
        "--mode",
        choices=("audit", "no-core", "inspect-core"),
        default="audit",
        help=(
            "audit: submit and exit; no-core: disable dumps then abort; "
            "inspect-core: permit a core, rely on MADV_DONTDUMP, then abort"
        ),
    )
    parser.add_argument(
        "--pause",
        action="store_true",
        help="Pause before audit so /proc/<pid>/smaps can be inspected.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if platform.system() != "Linux":
        raise SystemExit("This test must run on Linux.")
    if (args.wallet is None) == (args.identity is None):
        raise SystemExit("Provide exactly one of --wallet or --identity.")

    # inspect-core deliberately disables process-wide hardening so the kernel can
    # produce a core. The key mapping itself remains locked and MADV_DONTDUMP.
    harden = args.mode != "inspect-core"
    if args.identity is not None:
        wallet = Wallet.from_secure_identity(
            args.identity,
            sentinel_host=args.sentinel,
            require_lock=True,
            harden_process=harden,
            include_creds=True,
        )
        remote = True
    else:
        wallet = Wallet.from_wallet_file(
            args.wallet,
            secure=True,
            require_lock=True,
            harden_process=harden,
        )
        remote = False

    print(f"PID: {os.getpid()}")
    print(f"Mode: {args.mode}")
    print(f"Source: {'identity' if remote else 'wallet file'}")
    print("Security status:", wallet.security_status())
    print("Organization:", wallet.org)
    print(f"Address: {wallet.get_address()}")
    print(f"Core dumps process-wide disabled: {harden}")

    if args.pause:
        print()
        print("In another terminal inspect the mappings with:")
        print(f"  grep -B 20 -A 5 'VmFlags:.*dd' /proc/{os.getpid()}/smaps")
        input("Press Enter to submit the audit...")

    agent = WeilAgent(None, wallet=wallet, sentinel_host=args.sentinel)
    log = json.dumps(
        {
            "test": "secure-wallet-linux",
            "source": "identity" if remote else "wallet-file",
            "mode": args.mode,
            "pid": os.getpid(),
            "timestamp": int(time.time()),
        }
    )
    result = agent.audit(log)
    print(
        "Audit result:",
        f"status={result.status.value}",
        f"block={result.block_height}",
        f"batch={result.batch_id}",
        flush=True,
    )

    if args.mode == "audit":
        wallet.close()
        print("PASS: audit completed and secure wallet was closed.")
        return

    if args.mode == "no-core":
        print("Aborting now. No core dump should be created.", flush=True)
    else:
        if remote:
            print(
                "Aborting now. A core should be created, but the masked wallet "
                "JSON / AWS credentials must not be present in it.",
                flush=True,
            )
        else:
            print(
                "Aborting now. A core should be created, but the wallet/xprv/key "
                "must not be present in it.",
                flush=True,
            )
    os.abort()


if __name__ == "__main__":
    main()

