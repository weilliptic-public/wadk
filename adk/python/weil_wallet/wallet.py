"""Wallet primitives for WeilChain.

This module supports multi-account wallets backed by secp256k1 keys.

- PrivateKey: load a hex-encoded private key from disk or string.
- Account: a secp256k1 keypair with an associated (sentinel-minted) address.
- Wallet: multi-account wallet loaded from a wallet.wc file, supporting:
    - Derived accounts: HD-derived from the xprv stored in wallet.wc.
    - External accounts: imported accounts with their own secret keys.
    - Account switching via set_index().

Signing uses secp256k1 ECDSA over the SHA-256 digest of the input.
"""

import hashlib
import hmac as _hmac
import json
import struct
from dataclasses import dataclass
from pathlib import Path
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
    """Multi-account secp256k1 wallet for the WeilChain platform.

    Backwards compatible with the older single-account form:
    `Wallet(private_key)` creates a one-account external wallet.
    """

    __slots__ = ("_derived_accounts", "_added_accounts", "_current_account_index")

    def __init__(self, private_key: PrivateKey, account_address: str | None = None) -> None:
        account = Account.from_private_key_and_address(private_key, account_address or "")
        self._derived_accounts: list[Account] = []
        self._added_accounts: list[Account] = [account]
        self._current_account_index = SelectedAccount("external", 0)

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
        w._derived_accounts = []
        w._added_accounts = [account]
        w._current_account_index = SelectedAccount("external", 0)
        return w

    def add_account_from_export_file(self, path: Union[str, Path]) -> None:
        """Append an additional account export; does not change selection."""
        self._added_accounts.append(account_from_export_file(path))

    @classmethod
    def from_wallet_file(cls, path: Union[str, Path]) -> "Wallet":
        """Load a Wallet from a wallet.wc file.

        Derived account secret keys are re-derived from the stored xprv.
        External account secret keys are read directly from the file.

        Raises:
            ValueError: If the file type is not 'wallet' or contains no accounts.
            IndexError: If the selected_account index is out of bounds.
        """
        data = json.loads(Path(path).read_text())
        if data.get("type") != "wallet":
            raise ValueError(f"expected file type 'wallet', got '{data.get('type')}'")

        derived_entries = data.get("derived_accounts", []) or []
        external_entries = data.get("external_accounts", []) or []

        if not derived_entries and not external_entries:
            raise ValueError("wallet file contains no accounts")

        xprv_str = data["xprv"]
        master_key, master_chain = _decode_xprv(xprv_str)
        account_key, account_chain = _resolve_account_level_key(
            master_key, master_chain, derived_entries
        )

        derived_accounts: list[Account] = []
        for entry in derived_entries:
            child_key, _ = _bip32_derive_child(
                account_key, account_chain, int(entry["index"]), hardened=False
            )
            derived_accounts.append(Account(child_key, entry["account_address"]))

        added_accounts: list[Account] = []
        for entry in external_entries:
            sk_bytes = bytes.fromhex(entry["secret_key"])
            added_accounts.append(Account(sk_bytes, entry["account_address"]))

        sel = data.get("selected_account", {"type": "derived", "index": 0}) or {}
        kind = sel.get("type", "derived")
        idx = int(sel.get("index", 0))

        if kind == "external":
            if idx >= len(added_accounts):
                raise IndexError(
                    f"selected external account index {idx} out of bounds (have {len(added_accounts)})"
                )
            current = SelectedAccount("external", idx)
        else:
            if idx >= len(derived_accounts):
                raise IndexError(
                    f"selected derived account index {idx} out of bounds (have {len(derived_accounts)})"
                )
            current = SelectedAccount("derived", idx)

        w = cls.__new__(cls)
        w._derived_accounts = derived_accounts
        w._added_accounts = added_accounts
        w._current_account_index = current
        return w

    def set_index(self, selected: "SelectedAccount") -> None:
        """Switch active account. Raises if out of bounds."""
        if selected.account_type == "derived":
            if selected.index < 0 or selected.index >= len(self._derived_accounts):
                raise ValueError(
                    f"derived account index {selected.index} out of bounds "
                    f"(have {len(self._derived_accounts)} derived account(s))"
                )
        elif selected.account_type == "external":
            if selected.index < 0 or selected.index >= len(self._added_accounts):
                raise ValueError(
                    f"external account index {selected.index} out of bounds "
                    f"(have {len(self._added_accounts)} external account(s))"
                )
        else:
            raise ValueError(f"unknown account type: {selected.account_type}")

        self._current_account_index = selected

    def external_account_count(self) -> int:
        return len(self._added_accounts)

    def derived_account_count(self) -> int:
        return len(self._derived_accounts)

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
        sel = self._current_account_index
        if sel.account_type == "derived":
            return self._derived_accounts[sel.index]
        if sel.account_type == "external":
            return self._added_accounts[sel.index]
        raise ValueError(f"unknown account type: {sel.account_type}")


@dataclass(frozen=True)
class SelectedAccount:
    account_type: Literal["derived", "external"]
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


# ── BIP32 helpers for wallet.wc ───────────────────────────────────────────────

_SECP256K1_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141


def _decode_xprv(xprv_str: str) -> tuple[bytes, bytes]:
    """Decode a base58check xprv string and return (key_bytes_32, chain_code_32)."""
    from bip_utils import Base58Decoder

    raw = Base58Decoder.CheckDecode(xprv_str)
    if len(raw) != 78:
        raise ValueError(f"Invalid xprv: expected 78 decoded bytes, got {len(raw)}")
    chain_code = bytes(raw[13:45])
    key_bytes = bytes(raw[46:78])
    return key_bytes, chain_code


def _bip32_derive_child(
    parent_key: bytes, parent_chain: bytes, index: int, hardened: bool
) -> tuple[bytes, bytes]:
    """Derive one BIP32 child private key, returning (child_key, child_chain_code)."""
    if hardened:
        data = b"\x00" + parent_key + struct.pack(">I", index + 0x80000000)
    else:
        compressed_pub = Secp256k1PrivateKey(parent_key).public_key.format(compressed=True)
        data = compressed_pub + struct.pack(">I", index)

    I = _hmac.new(parent_chain, data, hashlib.sha512).digest()
    IL, IR = I[:32], I[32:]
    child_int = (int.from_bytes(IL, "big") + int.from_bytes(parent_key, "big")) % _SECP256K1_N
    return child_int.to_bytes(32, "big"), IR


def _resolve_account_level_key(
    master_key: bytes, master_chain: bytes, derived_entries: list
) -> tuple[bytes, bytes]:
    """Return (key, chain_code) at the account derivation level.

    If deriving child 0 directly matches the first entry's stored public_key,
    the xprv is already at account level. Otherwise traverse m/44'/9345'/0'/0 first.
    """
    if not derived_entries:
        return master_key, master_chain

    first = derived_entries[0]
    child_key, _ = _bip32_derive_child(master_key, master_chain, int(first["index"]), hardened=False)
    pk_hex = Secp256k1PrivateKey(child_key).public_key.format(compressed=True).hex()

    if pk_hex == first["public_key"]:
        return master_key, master_chain

    key, chain = master_key, master_chain
    for idx, hardened in [(44, True), (9345, True), (0, True), (0, False)]:
        key, chain = _bip32_derive_child(key, chain, idx, hardened=hardened)
    return key, chain


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
