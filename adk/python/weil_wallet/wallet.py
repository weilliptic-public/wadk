"""Wallet primitives for WeilChain.

This module supports multi-account wallets backed by secp256k1 keys.

- PrivateKey: load a hex-encoded private key from disk or string.
- Account: a secp256k1 keypair with an associated (sentinel-minted) address.
- Wallet: holds multiple accounts and supports switching via SelectedAccount.

Signing uses secp256k1 ECDSA over the SHA-256 digest of the input.
"""

from pathlib import Path
from dataclasses import dataclass
import json
from typing import Literal, Union

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
    """Multi-account secp256k1 wallet for the WeilChain platform."""

    __slots__ = ("_external_accounts", "_current")

    def __init__(self, private_key: PrivateKey, account_address: str | None = None) -> None:
        account = Account.from_private_key_and_address(private_key, account_address)
        self._external_accounts: list[Account] = [account]
        self._current = SelectedAccount("external", 0)

    @property
    def secret_key(self) -> Secp256k1PrivateKey:
        """Return the currently selected account's secp256k1 secret key."""
        return self._current_account().secret_key

    def get_public_key(self) -> Secp256k1PublicKey:
        """Return the currently selected account's secp256k1 public key."""
        return self._current_account().public_key

    def get_address(self) -> str:
        """Return the currently selected account's sentinel-minted address."""
        return self._current_account().account_address

    @classmethod
    def from_account_export_file(cls, path: Union[str, Path]) -> "Wallet":
        account = account_from_export_file(path)
        w = cls.__new__(cls)
        w._external_accounts = [account]
        w._current = SelectedAccount("external", 0)
        return w

    def add_account_from_export_file(self, path: Union[str, Path]) -> None:
        """Append an additional account export; does not change selection."""
        self._external_accounts.append(account_from_export_file(path))

    def set_index(self, selected: "SelectedAccount") -> None:
        """Switch active account. Raises if out of bounds."""
        if selected.account_type != "external":
            raise ValueError(f"unsupported account type: {selected.account_type}")
        if selected.index < 0 or selected.index >= len(self._external_accounts):
            raise ValueError(
                f"external account index {selected.index} out of bounds "
                f"(have {len(self._external_accounts)} external account(s))"
            )
        self._current = selected

    def external_account_count(self) -> int:
        return len(self._external_accounts)

    def sign(self, buf: bytes) -> str:
        """Sign buf with ECDSA secp256k1.

        The message is hashed with SHA-256, then signed. Returns hex-encoded
        64-byte compact signature (r || s), matching the Rust libsecp256k1 format.
        """
        digest = hash_sha256(buf)
        # coincurve returns DER; Rust uses compact 64-byte (r||s)
        der_signature = self._current_account().secret_key.sign(digest, hasher=None)
        compact = _der_signature_to_compact(der_signature)
        return compact.hex()

    def _current_account(self) -> "Account":
        if self._current.account_type != "external":
            raise ValueError(f"unsupported account type: {self._current.account_type}")
        return self._external_accounts[self._current.index]


@dataclass(frozen=True)
class SelectedAccount:
    account_type: Literal["external"]
    index: int


@dataclass
class Account:
    secret_key: Secp256k1PrivateKey
    public_key: Secp256k1PublicKey
    account_address: str

    @classmethod
    def from_private_key_and_address(
        cls, key: PrivateKey, account_address: str | None
    ) -> "Account":
        secret_key_bytes = bytes.fromhex(key._hex)
        secret = Secp256k1PrivateKey(secret_key_bytes)
        pub = secret.public_key
        if not account_address:
            # Fallback for backwards compatibility, but sentinel-minted addresses
            # should be provided via export files.
            account_address = ""
        return cls(secret_key=secret, public_key=pub, account_address=account_address)


def account_from_export_file(path: Union[str, Path]) -> Account:
    path = Path(path)
    raw = path.read_text(encoding="utf-8")
    data = json.loads(raw)
    if data.get("type") != "account":
        raise ValueError(f"expected export type 'account', got '{data.get('type')}'")
    account = data.get("account") or {}
    key_hex = account.get("secret_key", "")
    addr = account.get("account_address", "")
    if not addr:
        raise ValueError("account export missing account_address")
    pk = PrivateKey(key_hex)
    return Account.from_private_key_and_address(pk, addr)


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
