"""Mnemonic-based wallet with BIP32 derived accounts.

Creates multiple wallet addresses from a master key using the
derivation path : m/44'/9345'/0'/0 with account index.
"""

import base64
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional, Union

from bip_utils import (
    Bip32Slip10Secp256k1,
    Bip39MnemonicGenerator,
    Bip39SeedGenerator,
    Bip39WordsNum,
)

from .wallet import PrivateKey, Wallet

DERIVATION_PATH = "m/44'/9345'/0'/0"
_STORED_WALLET_VERSION = 1


def _keccak256(data: bytes) -> bytes:
    """Keccak-256 hash (used for derived-account address).

    Requires eth_hash (installed via eth-hash[pycryptodome] or similar).
    Note: hashlib.sha3_256 is NOT a drop-in replacement — it uses a different
    final padding and produces different output, which would silently produce
    wrong wallet addresses.
    """
    from eth_hash.auto import keccak  # ImportError if eth_hash not installed
    return keccak(data)


def pubkey_to_derived_address(pubkey_bytes: bytes) -> str:
    """
    Derived-account address from uncompressed public key.
    Keccak256 of pubkey (minus 0x04 prefix), last 20 bytes, 0x-prefixed.
    Matches server-side TxnUtils::get_address_from_public_key for derived accounts.
    """
    if len(pubkey_bytes) == 65 and pubkey_bytes[0] == 0x04:
        pubkey_bytes = pubkey_bytes[1:]  # drop 0x04 prefix
    addr = _keccak256(pubkey_bytes)[-20:]
    return "0x" + addr.hex()


@dataclass
class WalletAccount:
    """A single derived account: key material and derived address (0x-prefixed)."""

    private_key: bytes
    public_key: bytes
    address: str

    def to_weil_wallet(self) -> Wallet:
        """Return a Weil SDK Wallet that can sign and be used with WeilClient."""
        pk = PrivateKey.from_bytes(self.private_key)
        return Wallet(pk)


class MnemonicWallet:
    """
    Wallet created from a BIP39 mnemonic with BIP32 derivation.
    Derives accounts at m/44'/9345'/0'/0/{index}.
    """

    def __init__(self, mnemonic: str, master_key: Any) -> None:
        """Initialise from a BIP39 mnemonic and a pre-derived BIP32 master key object."""
        self.mnemonic = mnemonic
        self._master_key = master_key
        self._accounts: dict[int, WalletAccount] = {}

    def derive_account(self, index: int) -> WalletAccount:
        """Derive the account at the given index (same as server-side)."""
        if index in self._accounts:
            return self._accounts[index]
        child = self._master_key.ChildKey(index)
        priv_bytes = child.PrivateKey().Raw().ToBytes()
        pub_bytes = child.PublicKey().RawUncompressed().ToBytes()
        address = pubkey_to_derived_address(pub_bytes)
        account = WalletAccount(
            private_key=priv_bytes,
            public_key=pub_bytes,
            address=address,
        )
        self._accounts[index] = account
        return account

    def get_account(self, index: int) -> WalletAccount:
        """Alias for derive_account."""
        return self.derive_account(index)

    def get_address(self, index: int = 0) -> str:
        """Return the derived address (0x-prefixed) for account at index (default 0)."""
        return self.derive_account(index).address

    def store_wallet(self, path: Union[str, Path]) -> None:
        """
        Save an encoded, serialized version of this wallet to a file.

        The stored data includes the mnemonic and derivation path so that
        load_wallet(path) recreates the same wallet and the same derived
        accounts (derive_account(0), derive_account(1), ...) as before.
        """
        path = Path(path)
        payload = {
            "version": _STORED_WALLET_VERSION,
            "mnemonic": self.mnemonic,
            "derivation_path": DERIVATION_PATH,
        }
        serialized = json.dumps(payload, separators=(",", ":"))
        encoded = base64.b64encode(serialized.encode("utf-8")).decode("ascii")
        path.write_text(encoded, encoding="utf-8")


def load_wallet(path: Union[str, Path]) -> MnemonicWallet:
    """
    Load a wallet from a file previously saved with store_wallet().

    Returns a MnemonicWallet with the same mnemonic and derivation path,
    so derived accounts (derive_account(0), derive_account(1), ...) are
    the same as when the wallet was stored.
    """
    path = Path(path)
    encoded = path.read_text(encoding="utf-8").strip()
    serialized = base64.b64decode(encoded).decode("utf-8")
    payload = json.loads(serialized)
    if payload.get("version") != _STORED_WALLET_VERSION:
        raise ValueError(
            f"Unsupported stored wallet version: {payload.get('version')}; "
            f"expected {_STORED_WALLET_VERSION}"
        )
    mnemonic = payload["mnemonic"]
    seed_bytes = Bip39SeedGenerator(mnemonic).Generate()
    master = Bip32Slip10Secp256k1.FromSeed(seed_bytes)
    derived_path = payload.get("derivation_path", DERIVATION_PATH)
    derived = master.DerivePath(derived_path)
    return MnemonicWallet(mnemonic, derived)


def create_wallet(
    mnemonic: Optional[str] = None,
    generate_mnemonic: bool = True,
) -> MnemonicWallet:
    """
    Create a wallet from a mnemonic or generate a new one.

    Behaviour:
    - If mnemonic is provided -> use it.
    - Else if generate_mnemonic is True -> generate a new 24-word mnemonic.
    - Else -> raise ValueError.

    Derives the master key at m/44'/9345'/0'/0 (same as server).
    """
    if mnemonic is None and not generate_mnemonic:
        raise ValueError("Either mnemonic or generate_mnemonic must be provided")

    if mnemonic is None:
        mnemonic_obj = Bip39MnemonicGenerator().FromWordsNumber(Bip39WordsNum.WORDS_NUM_24)
        mnemonic = str(mnemonic_obj)

    seed_bytes = Bip39SeedGenerator(mnemonic).Generate()
    master = Bip32Slip10Secp256k1.FromSeed(seed_bytes)
    derived = master.DerivePath(DERIVATION_PATH)

    return MnemonicWallet(mnemonic, derived)
