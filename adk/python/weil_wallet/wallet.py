"""Wallet primitives for WeilChain.

- PrivateKey: load a hex-encoded private key from disk or string.
- Wallet: signing account from a private key; exposes secp256k1 public key and sign().

Signing uses secp256k1 ECDSA over the SHA-256 digest of the input.
"""

from pathlib import Path
from typing import Union

from coincurve import PrivateKey as Secp256k1PrivateKey, PublicKey as Secp256k1PublicKey

from .utils import hash_sha256


class PrivateKey:
    """Represents the private key associated with your account.

    Use the same key as the official Weilliptic CLI or browser wallet
    for the same on-chain identity.
    """

    __slots__ = ("_hex",)

    def __init__(self, hex_str: str) -> None:
        hex_trimmed = hex_str.strip()
        if not hex_trimmed:
            raise ValueError("private key is empty")
        if len(hex_trimmed) % 2 != 0 or not all(c in "0123456789abcdefABCDEF" for c in hex_trimmed):
            raise ValueError("private key is not a valid hexadecimal string")
        self._hex = hex_trimmed

    @classmethod
    def from_file(cls, path: Union[str, Path]) -> "PrivateKey":
        """Load a hex-encoded private key from a file."""
        path = Path(path)
        content = path.read_text().strip()
        if not content:
            raise ValueError("private key file is empty")
        return cls(content)

    @classmethod
    def from_hex(cls, hex_str: str) -> "PrivateKey":
        """Create from a hex string (convenience alias for constructor)."""
        return cls(hex_str)

    @classmethod
    def from_bytes(cls, key_bytes: bytes) -> "PrivateKey":
        """Create from raw private key bytes (e.g. from BIP32 derivation)."""
        return cls(key_bytes.hex())


class Wallet:
    """secp256k1-backed wallet for the WeilChain platform."""

    __slots__ = ("_secret_key", "_public_key")

    def __init__(self, private_key: PrivateKey) -> None:
        secret_key_bytes = bytes.fromhex(private_key._hex)
        self._secret_key = Secp256k1PrivateKey(secret_key_bytes)
        self._public_key = self._secret_key.public_key

    @property
    def secret_key(self) -> Secp256k1PrivateKey:
        """Return the account's secp256k1 secret key. Handle with care."""
        return self._secret_key

    def get_public_key(self) -> Secp256k1PublicKey:
        """Return the account's secp256k1 public key."""
        return self._public_key

    def sign(self, buf: bytes) -> str:
        """Sign buf with ECDSA secp256k1.

        The message is hashed with SHA-256, then signed. Returns hex-encoded
        64-byte compact signature (r || s), matching the Rust libsecp256k1 format.
        """
        digest = hash_sha256(buf)
        # coincurve returns DER; Rust uses compact 64-byte (r||s)
        der_signature = self._secret_key.sign(digest, hasher=None)
        compact = _der_signature_to_compact(der_signature)
        return compact.hex()


def _der_signature_to_compact(der: bytes) -> bytes:
    """Convert DER-encoded ECDSA signature to 64-byte compact (r||s)."""
    if len(der) < 8:
        raise ValueError("invalid DER signature length")
    # DER: 0x30 [total] 0x02 [r_len] [r...] 0x02 [s_len] [s...]
    if der[0] != 0x30:
        raise ValueError("DER signature must start with 0x30")

    # Parse the outer length field (short or long form)
    if der[1] == 0x80:
        raise ValueError("indefinite length DER not supported")
    if der[1] < 128:
        i = 2
    else:
        num_len_bytes = der[1] & 0x7F
        if 2 + num_len_bytes > len(der):
            raise ValueError("DER signature too short for length encoding")
        i = 2 + num_len_bytes

    # Parse r
    if i + 1 >= len(der):
        raise ValueError("DER signature truncated before r tag")
    if der[i] != 0x02:
        raise ValueError("expected 0x02 tag for r component")
    r_len = der[i + 1]
    if i + 2 + r_len > len(der):
        raise ValueError("DER signature truncated in r component")
    r = der[i + 2 : i + 2 + r_len]
    i += 2 + r_len

    # Parse s
    if i + 1 >= len(der):
        raise ValueError("DER signature truncated before s tag")
    if der[i] != 0x02:
        raise ValueError("expected 0x02 tag for s component")
    s_len = der[i + 1]
    if i + 2 + s_len > len(der):
        raise ValueError("DER signature truncated in s component")
    s = der[i + 2 : i + 2 + s_len]

    # DER encodes positive integers with a leading 0x00 when the high bit is set.
    # Strip that padding byte before converting to the 32-byte compact form.
    if len(r) == 33 and r[0] == 0x00:
        r = r[1:]
    if len(s) == 33 and s[0] == 0x00:
        s = s[1:]
    if len(r) > 32:
        raise ValueError(f"r component is {len(r)} bytes, expected <= 32")
    if len(s) > 32:
        raise ValueError(f"s component is {len(s)} bytes, expected <= 32")

    return r.rjust(32, b"\x00") + s.rjust(32, b"\x00")
