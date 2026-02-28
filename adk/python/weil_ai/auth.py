"""Weil auth utilities — signing and verification for HTTP request authentication.

Client side:
    build_auth_headers(wallet) → dict
        Build the X-Wallet-Address / X-Signature / X-Message / X-Timestamp headers
        using the weil_wallet signing scheme.

Server side:
    verify_weil_signature(wallet_address, signature_hex, message, timestamp) → bool
        Verify the four auth headers without any framework dependency.
"""

from __future__ import annotations

import hashlib
import json
import time

import coincurve

from weil_wallet.utils import get_address_from_public_key
from weil_wallet.wallet import Wallet

# Maximum tolerated age of a request timestamp (seconds).
# Matches _ALLOWED_TIMESTAMP_DRIFT used by weil_middleware().
MAX_TIMESTAMP_AGE_SECONDS: int = 300  # 5 minutes


def build_auth_headers(wallet: Wallet) -> dict:
    """Build the four auth headers required by weil_middleware().

    Signs a canonical JSON payload of ``{"timestamp": <ts>}`` with the wallet
    private key so the server can recover the signer address and verify ownership.

    Args:
        wallet: Signing wallet (holds the private key).

    Returns:
        Dict with keys ``X-Wallet-Address``, ``X-Signature``, ``X-Message``,
        and ``X-Timestamp``.
    """
    timestamp = str(int(time.time()))
    args = {"timestamp": timestamp}
    json_str = json.dumps(args, separators=(",", ":"), sort_keys=True)
    signature = wallet.sign(json_str.encode("utf-8"))
    address = get_address_from_public_key(wallet.get_public_key())

    return {
        "X-Wallet-Address": address,
        "X-Signature": signature,
        "X-Message": json_str,
        "X-Timestamp": timestamp,
    }


def verify_weil_signature(
    wallet_address: str,
    signature_hex: str,
    message: str,
    timestamp: str,
    max_age_seconds: int = MAX_TIMESTAMP_AGE_SECONDS,
) -> bool:
    """Verify the four auth headers produced by build_auth_headers().

    Steps (mirrors verify_me in transaction.rs):
      1. Reject stale timestamps (anti-replay).
      2. Compute SHA256 of the raw message bytes — this is the digest that
         weil_wallet.sign() signs over.
      3. Decode the 64-byte compact signature (r || s).
      4. Recover the secp256k1 public key from (signature, digest).
         The compact format carries no recovery-id, so we try 0 and 1.
      5. Derive the address from the recovered key and compare with
         X-Wallet-Address.

    Args:
        wallet_address: X-Wallet-Address header value.
        signature_hex:  X-Signature header value (hex of 64 compact bytes).
        message:        X-Message header value (the JSON string that was signed).
        timestamp:      X-Timestamp header value (Unix seconds, as string).
        max_age_seconds: Maximum tolerated age of the timestamp. Defaults to
                         ``MAX_TIMESTAMP_AGE_SECONDS`` (300 s / 5 min).

    Returns:
        ``True`` if the signature is valid and the address matches.
    """
    # 1. Timestamp freshness
    try:
        ts = int(timestamp)
    except (ValueError, TypeError):
        return False
    if abs(int(time.time()) - ts) > max_age_seconds:
        return False

    # 2. SHA256 of the message — mirrors hash_sha256(verify_payload.as_bytes())
    digest = hashlib.sha256(message.encode("utf-8")).digest()

    # 3. Decode compact 64-byte signature (r || s)
    try:
        sig_bytes = bytes.fromhex(signature_hex)
    except ValueError:
        return False
    if len(sig_bytes) != 64:
        return False

    # 4 + 5. Recover public key and check address.
    # coincurve expects a 65-byte recoverable signature: r(32) || s(32) || v(1)
    # v is the recovery id. We try 0 and 1 (2/3 are valid only for edge-case keys).
    for recovery_id in (0, 1):
        try:
            recoverable_sig = sig_bytes + bytes([recovery_id])
            pub = coincurve.PublicKey.from_signature_and_message(
                recoverable_sig,
                digest,
                hasher=None,  # digest is already SHA256-hashed; don't hash again
            )
            # SHA256 of uncompressed key mirrors get_address_from_public_key()
            derived = hashlib.sha256(pub.format(compressed=False)).hexdigest()
            if derived == wallet_address.lower():
                return True
        except Exception:
            continue

    return False
