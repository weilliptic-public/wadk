"""Cryptographic and utility helpers.

- hash_sha256: SHA-256 over a byte slice
- get_address_from_public_key: derive address (hex SHA-256 of compressed secp256k1 pubkey)
- current_time_millis: Unix epoch time in milliseconds
- compress: JSON-serialize a value and GZIP-compress it
"""

import hashlib
import gzip
import json
import time
from typing import Any

from coincurve import PublicKey as Secp256k1PublicKey


def hash_sha256(buf: bytes) -> bytes:
    """Compute the SHA-256 digest of buf. Returns 32 bytes."""
    return hashlib.sha256(buf).digest()


def get_address_from_public_key(public_key: Secp256k1PublicKey) -> str:
    """Derive address string from a secp256k1 public key.

    The address is the hex-encoded SHA-256 of the key's **uncompressed** (full)
    bytes, matching the Rust SDK (libsecp256k1 PublicKey::serialize() returns
    FULL_PUBLIC_KEY_SIZE, 65 bytes).
    """
    full = public_key.format(compressed=False)
    addr = hash_sha256(full)
    return addr.hex()


def current_time_millis() -> float:
    """Return current Unix time in milliseconds as float."""
    return time.time() * 1000.0


def timestamp() -> int:
    """Return current Unix time in milliseconds as integer."""
    return int(current_time_millis())


def compress(value: Any) -> bytes:
    """Serialize value to JSON and GZIP-compress the bytes."""
    json_str = json.dumps(value, separators=(",", ":"))
    return gzip.compress(json_str.encode("utf-8"))


def value_to_sorted_dict(value: Any) -> dict[str, Any]:
    """Convert a JSON-serializable dict into one with sorted keys (canonical form)."""
    if not isinstance(value, dict):
        raise TypeError(f"expected dict, got {type(value).__name__}")
    return dict(sorted(value.items()))
