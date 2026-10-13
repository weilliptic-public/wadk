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
import inspect
import json
import secrets
import time
from collections.abc import Mapping
from typing import Any, Callable, Optional

import coincurve

from weil_wallet.utils import get_address_from_public_key
from weil_wallet.wallet import Wallet

# Maximum tolerated age of a request timestamp (seconds).
# Matches _ALLOWED_TIMESTAMP_DRIFT used by weil_middleware().
MAX_TIMESTAMP_AGE_SECONDS: int = 300  # 5 minutes


def build_auth_headers(
    wallet: Wallet,
    signer: Optional[Callable[[bytes], str]] = None,
    body: Any = None,
) -> dict:
    """Build the four auth headers required by weil_middleware().

    Signs a canonical JSON payload of ``{"timestamp": <ts>}`` with the wallet
    private key so the server can recover the signer address and verify
    ownership. ``X-Wallet-Address`` carries the wallet's full account address;
    only its trailing 64 chars are checked against the recovered signing key.

    Args:
        wallet: Signing wallet (holds the private key).
        signer: Optional callable that takes the message bytes and returns a
            hex signature. Defaults to ``wallet.sign`` (local signing). Pass
            a Sentinel-backed signer for agent identity wallets.

    Returns:
        Dict with keys ``X-Wallet-Address``, ``X-Signature``, ``X-Message``,
        and ``X-Timestamp``.
    """
    json_str, args = _auth_payload(wallet, body)
    sign = signer or wallet.sign
    signature = sign(json_str.encode("utf-8"))
    return _auth_headers(wallet, json_str, args, signature)


async def abuild_auth_headers(
    wallet: Wallet,
    signer: Optional[Callable[[bytes], Any]] = None,
    body: Any = None,
) -> dict:
    """Async build_auth_headers: *signer* may be a coroutine function.

    Use this from async code. It awaits the signer on the caller's own event
    loop, where build_auth_headers would have to bridge through _run_sync and
    run it on a new one.
    """
    json_str, args = _auth_payload(wallet, body)
    if signer is None:
        signature = wallet.sign(json_str.encode("utf-8"))
    else:
        signature = signer(json_str.encode("utf-8"))
        if inspect.isawaitable(signature):
            signature = await signature
    return _auth_headers(wallet, json_str, args, signature)


def _auth_payload(wallet: Wallet, body: Any) -> tuple[str, dict]:
    """Build the canonical message to sign. Pure; no I/O.

    Shared by the sync and async builders so both produce byte-identical
    messages -- the server re-derives the same bytes to verify.
    """
    timestamp = str(int(time.time()))
    # The canonical (sentinel-minted) address carries the pod counter, which
    # cannot be derived from the public key, so it must be transmitted. It goes
    # inside the signed payload -- never as a bare header -- so the server can
    # trust it. See full_address_from_headers().
    args = {"timestamp": timestamp, "address": wallet.get_address()}
    if body is not None:
        args["body_sha256"] = _body_sha256(body)
        args["nonce"] = secrets.token_hex(16)
    return json.dumps(args, separators=(",", ":"), sort_keys=True), args


def _auth_headers(wallet: Wallet, json_str: str, args: dict, signature: str) -> dict:
    headers = {
        "X-Wallet-Address": get_address_from_public_key(wallet.get_public_key()),
        "X-Signature": signature,
        "X-Message": json_str,
        "X-Timestamp": args["timestamp"],
    }
    if "body_sha256" in args:
        headers["X-Body-SHA256"] = args["body_sha256"]
        headers["X-Nonce"] = args["nonce"]
    return headers


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
      5. Derive the signing-key address from the recovered key and compare it
         with the trailing 64 chars of X-Wallet-Address.

    Args:
        wallet_address: X-Wallet-Address header value — the caller's full
                        account address. Only its trailing 64 chars are
                        compared (the signing-key address).
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
            # SHA256 of the uncompressed key is the 64-char signing-key
            # address; the header's trailing 64 chars must match it.
            derived = hashlib.sha256(pub.format(compressed=False)).hexdigest()
            if derived == wallet_address.lower()[-64:]:
                return True
        except Exception:
            continue

    return False


def verify_weil_request(
    headers: Mapping[str, str],
    body: Any = None,
    max_age_seconds: int = MAX_TIMESTAMP_AGE_SECONDS,
) -> bool:
    """Verify Weil auth headers and, when present, bind them to *body*.

    ``build_auth_headers(..., body=payload)`` signs a canonical message
    containing ``body_sha256``. This verifier rejects requests where the signed
    body hash and the actual request body differ, so request tampering is caught
    before the application sees the payload.
    """

    def get(name: str) -> Optional[str]:
        if name in headers:
            return headers[name]
        lower = name.lower()
        for key, value in headers.items():
            if key.lower() == lower:
                return value
        return None

    wallet_address = get("X-Wallet-Address")
    signature_hex = get("X-Signature")
    message = get("X-Message")
    timestamp = get("X-Timestamp")
    if not verify_weil_signature(
        wallet_address,
        signature_hex,
        message,
        timestamp,
        max_age_seconds=max_age_seconds,
    ):
        return False

    if body is None:
        return True

    try:
        signed = json.loads(message)
    except (TypeError, ValueError):
        return False

    signed_hash = signed.get("body_sha256")
    header_hash = get("X-Body-SHA256")
    actual_hash = _body_sha256(body)
    return bool(signed_hash) and signed_hash == header_hash == actual_hash


def full_address_from_headers(headers: Mapping[str, str]) -> Optional[str]:
    """Return the signed canonical (72-hex) wallet address, if trustworthy.

    ``build_auth_headers`` signs the sentinel-minted address alongside the
    timestamp. Only the SHA-256 suffix of that address is re-derivable from the
    signature, so the pod-counter prefix is accepted only when the rest matches
    the verified ``X-Wallet-Address``. That binds the claimed pod counter to the
    key whose ownership the signature already proved.

    Returns None when the header set predates this field or fails the binding
    check; callers should then fall back to ``X-Wallet-Address``.

    Call only on headers that already passed verify_weil_request().
    """

    def get(name: str) -> Optional[str]:
        if name in headers:
            return headers[name]
        lower = name.lower()
        for key, value in headers.items():
            if key.lower() == lower:
                return value
        return None

    message = get("X-Message")
    short = (get("X-Wallet-Address") or "").lower()
    if not message or not short:
        return None
    try:
        signed = json.loads(message)
    except (TypeError, ValueError):
        return None
    if not isinstance(signed, dict):
        return None
    full = signed.get("address")
    if not isinstance(full, str) or not full:
        return None
    return full if full.lower().endswith(short) else None


def _canonical_body_bytes(body: Any) -> bytes:
    if isinstance(body, bytes):
        return body
    if isinstance(body, str):
        return body.encode("utf-8")
    return json.dumps(body, separators=(",", ":"), sort_keys=True, default=repr).encode(
        "utf-8"
    )


def _body_sha256(body: Any) -> str:
    return hashlib.sha256(_canonical_body_bytes(body)).hexdigest()
