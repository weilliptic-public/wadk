"""Wallet primitives for WeilChain.

- Account: a secp256k1 keypair with an associated address.
- SelectedAccount: identifies which account in a Wallet is currently active.
- Wallet: multi-account wallet loaded from a wallet.wc file, supporting:
    - Derived accounts: HD-derived from the xprv stored in wallet.wc.
    - External accounts: imported accounts with their own secret keys.
    - Account switching via set_index().

Address format:
- All account addresses are sentinel-minted 72-char hex strings (embeds an
  obfuscated weilpod_counter). Cannot be derived from the private key alone.
"""

import hashlib
import hmac as _hmac
import json
import struct
import warnings
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional, Union

from coincurve import PrivateKey as Secp256k1PrivateKey, PublicKey as Secp256k1PublicKey
from .secure_wallet import SecureAccount, harden_process as apply_hardening
from .utils import hash_sha256


# ── Organization info ────────────────────────────────────────────────────────

@dataclass
class OrgInfo:
    """Organization membership linked via ``wallet link-org``.

    Used for v1 wallet files (backward compat) and as the public return type.
    """
    name: str
    subgroup: Optional[str] = None
    purpose: str = ""


@dataclass
class OrgMembership:
    """v2 org membership entry — purpose is resolved at runtime."""
    org: str
    subgroup: str = ""


# ── SelectedAccount ───────────────────────────────────────────────────────────

@dataclass
class SelectedAccount:
    """Identifies which account in the wallet is currently active.

    Use the class methods Derived(i) and External(i) as constructors,
    mirroring the Rust enum variants.
    """
    kind: str   # "derived" or "external"
    index: int

    @classmethod
    def Derived(cls, index: int) -> "SelectedAccount":
        """A BIP32 HD-derived account at the given index."""
        return cls(kind="derived", index=index)

    @classmethod
    def External(cls, index: int) -> "SelectedAccount":
        """An externally imported account at the given index."""
        return cls(kind="external", index=index)

    def __str__(self) -> str:
        if self.kind == "derived":
            return f"Derived Account {self.index}"
        return f"External Account {self.index}"


# ── Account ───────────────────────────────────────────────────────────────────

class Account:
    """A single WeilChain account: secp256k1 keypair + address.

    API-key accounts are public-only: no secret key, signing is delegated to
    the Sentinel.
    """

    __slots__ = ("_secret_key", "_public_key", "_address")

    def __init__(
        self,
        secret_key_bytes: bytes | None = None,
        address: str = "",
        *,
        public_key_hex: str | None = None,
    ) -> None:
        """Build from a secret key, or from a public key for API-key accounts.

        Args:
            secret_key_bytes: Raw 32-byte secp256k1 secret key.
            address: Sentinel-minted 72-char hex account address.
            public_key_hex: Hex public key; used instead of ``secret_key_bytes``
                when the account has no local secret key.
        """
        if public_key_hex is not None:
            self._secret_key = None
            self._public_key = Secp256k1PublicKey(bytes.fromhex(public_key_hex))
        elif secret_key_bytes is not None:
            self._secret_key = Secp256k1PrivateKey(secret_key_bytes)
            self._public_key = self._secret_key.public_key
        else:
            raise ValueError("provide secret_key_bytes or public_key_hex")
        self._address = address

    def get_public_key(self) -> Secp256k1PublicKey:
        """Return the account's secp256k1 public key."""
        return self._public_key

    def get_address(self) -> str:
        """Return the sentinel-minted 72-char hex address."""
        return self._address

    def get_secret_key(self) -> Secp256k1PrivateKey:
        """Return the secret key, or ``None`` for public-only accounts."""
        return self._secret_key

    def sign(self, buf: bytes) -> str:
        """Sign buf with ECDSA secp256k1.

        The message is hashed with SHA-256, then signed. Returns hex-encoded
        64-byte compact signature (r || s), matching the Rust libsecp256k1 format.
        """
        digest = hash_sha256(buf)
        der_signature = self._secret_key.sign(digest, hasher=None)
        compact = _der_signature_to_compact(der_signature)
        return compact.hex()


# ── PrivateKey ────────────────────────────────────────────────────────────────

class PrivateKey:
    """Represents the private key associated with an account.

    Used by derived_wallet.py for mnemonic-based account construction.
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


# ── Wallet ────────────────────────────────────────────────────────────────────

class Wallet:
    """Multi-account secp256k1 wallet for the WeilChain platform.

    Loaded from a wallet.wc file. Holds:
    - derived_accounts: HD-derived from the xprv stored in the file.
    - added_accounts: externally imported (with their own secret keys).

    All signing and address operations act on the currently selected account.
    Use set_index() to switch accounts at runtime.
    """

    def __init__(self) -> None:
        """Internal no-arg constructor; use the class-method constructors."""
        self._derived_accounts: list[Account] = []
        self._added_accounts: list[Account] = []
        self._current_account_index: SelectedAccount = SelectedAccount.Derived(0)
        self._org: Optional[OrgInfo] = None
        self._source_json: Optional[str] = None
        self._secure_api_key: bool = False

    # ── Constructors ──────────────────────────────────────────────────────────

    @classmethod
    def from_wallet_file(
        cls,
        path: Union[str, Path],
        *,
        secure: bool = False,
        require_lock: bool = True,
        harden_process: bool = True,
    ) -> "Wallet":
        """Load a Wallet from a wallet.wc file.

        Derived account secret keys are re-derived from the stored xprv.
        External account secret keys are read directly from the file.
        The active account is set from the selected_account field (defaults
        to the first derived account when absent).

        Args:
            path: Path to the wallet.wc file.
            secure: On Linux, parse the wallet and derive its selected account
                entirely in the native secure-memory backend. The resulting
                wallet is a selected-account snapshot; secret material and the
                complete account list are not exposed to Python.
            require_lock: With ``secure=True``, fail if protected mappings
                cannot be locked into RAM.
            harden_process: With ``secure=True``, disable core dumps before the
                wallet file is opened. Strong protection should set this True.

        Raises:
            ValueError: If the file type is not 'wallet' or contains no accounts.
            IndexError: If the selected_account index is out of bounds.
        """
        if not secure:
            return cls.from_wallet_json(Path(path).read_text())

        from .secure_wallet import SecureAccount, harden_process as apply_hardening

        if not harden_process:
            warnings.warn(
                "harden_process=False disables RLIMIT_CORE=0 and PR_SET_DUMPABLE=0. "
                "Secret key material may appear in core dumps. ",
                stacklevel=2,
            )
        elif harden_process:
            apply_hardening()
        account, (org_name, subgroup, purpose) = SecureAccount.from_wallet_file(
            path, require_lock=require_lock
        )
        w = cls()
        w._added_accounts = [account]
        w._current_account_index = SelectedAccount.External(0)
        if org_name:
            w._org = OrgInfo(
                name=org_name,
                subgroup=subgroup or None,
                purpose=purpose or "",
            )
        return w

    @classmethod
    def from_wallet_json(
        cls,
        content: Union[str, bytes, dict[str, Any]],
        masked: bool = False,
    ) -> "Wallet":
        """Load a Wallet from serialized wallet.wc JSON content.

        Args:
            content: Wallet JSON (string, bytes, or dict).
            masked: True when the wallet came from the Agent Registry API key
                flow — the xprv is a masked token that cannot be decoded
                locally, so accounts are built public-only (no secret keys)
                and signing is delegated to the Sentinel.
        """
        if isinstance(content, bytes):
            content = content.decode("utf-8")
        if isinstance(content, str):
            content = content.strip()
            if not content:
                raise ValueError("wallet JSON content is empty")
        data = content if isinstance(content, dict) else json.loads(content)
        if data.get("type") != "wallet":
            raise ValueError(
                f"expected file type 'wallet', got '{data.get('type')}'"
            )

        derived_entries = data.get("derived_accounts", [])
        external_entries = data.get("external_accounts", [])

        if not derived_entries and not external_entries:
            raise ValueError("wallet file contains no accounts")

        if masked:
            # API-key wallets delegate signing to the Sentinel via /sign_payload.
            derived_accounts = []
            for entry in derived_entries:
                derived_accounts.append(
                    Account(address=entry["account_address"], public_key_hex=entry["public_key"])
                )
            added_accounts = []
            for entry in external_entries:
                added_accounts.append(
                    Account(address=entry["account_address"], public_key_hex=entry["public_key"])
                )
        else:
            master_key, master_chain = _decode_xprv(data["xprv"])
            account_key, account_chain = _resolve_account_level_key(
                master_key, master_chain, derived_entries
            )

            derived_accounts = []
            for entry in derived_entries:
                child_key, _ = _bip32_derive_child(
                    account_key, account_chain, entry["index"], hardened=False
                )
                derived_accounts.append(Account(child_key, entry["account_address"]))

            added_accounts = []
            for entry in external_entries:
                sk_bytes = bytes.fromhex(entry["secret_key"])
                added_accounts.append(Account(sk_bytes, entry["account_address"]))

        sel = data.get("selected_account", {"type": "derived", "index": 0})
        kind = sel.get("type", "derived")
        idx = int(sel.get("index", 0))

        if kind == "external":
            if idx >= len(added_accounts):
                raise IndexError(
                    f"selected external account index {idx} out of bounds "
                    f"(have {len(added_accounts)})"
                )
            current = SelectedAccount.External(idx)
        else:
            if idx >= len(derived_accounts):
                raise IndexError(
                    f"selected derived account index {idx} out of bounds "
                    f"(have {len(derived_accounts)})"
                )
            current = SelectedAccount.Derived(idx)

        # ── Resolve org ───────────────────────────────────────────────────
        # Priority: per-account v2 orgs > top-level v2 orgs > v1 single org.
        sel_kind = sel.get("type", "derived")
        sel_idx = idx

        if sel_kind == "external":
            entry = external_entries[sel_idx] if sel_idx < len(external_entries) else {}
        else:
            entry = derived_entries[sel_idx] if sel_idx < len(derived_entries) else {}

        account_orgs_raw = entry.get("orgs", [])
        account_active_org = entry.get("active_org")

        if account_orgs_raw:
            # Per-account v2 orgs (preferred)
            oa_idx = account_active_org if account_active_org is not None else 0
            if oa_idx < len(account_orgs_raw):
                m = account_orgs_raw[oa_idx]
                sg = m.get("subgroup", "")
                org = OrgInfo(
                    name=m.get("org", ""),
                    subgroup=sg if sg else None,
                )
            else:
                org = None
        else:
            # Top-level v2 orgs list
            top_orgs = data.get("orgs", [])
            if top_orgs:
                tl_idx = data.get("active_org", 0) or 0
                if tl_idx < len(top_orgs):
                    m = top_orgs[tl_idx]
                    sg = m.get("subgroup", "")
                    org = OrgInfo(
                        name=m.get("org", ""),
                        subgroup=sg if sg else None,
                    )
                else:
                    org = None
            else:
                # v1 single org field
                v1_org = data.get("org")
                if v1_org and isinstance(v1_org, dict):
                    org = OrgInfo(
                        name=v1_org.get("name", ""),
                        subgroup=v1_org.get("subgroup"),
                        purpose=v1_org.get("purpose", ""),
                    )
                else:
                    org = None

        w = cls()
        w._derived_accounts = derived_accounts
        w._added_accounts = added_accounts
        w._current_account_index = current
        w._org = org
        if masked:
            w._source_json = (
                content
                if isinstance(content, str)
                else json.dumps(data, separators=(",", ":"))
            )
        return w

    """ AWS credentials are read natively from ``AWS_ACCESS_KEY_ID``,
    ``AWS_SECRET_ACCESS_KEY``, ``AWS_REGION``, and ``AWS_BUCKET_NAME``.
    The wallet response is received into mlocked memory and never exposed to
    HTTPX. The wallet is fetched with ``unmasked: true``, so the secret key
    is derived in secure memory and signing is done locally via the native
    backend — same as a wallet file with ``secure=True``. 
    """
    @classmethod
    def from_secure_api_key(
        cls,
        api_key: str,
        *,
        sentinel_host: str,
        verify: bool = True,
        require_lock: bool = True,
        harden_process: bool = True,
        include_creds: bool = False,
    ) -> "Wallet":
        """Fetch the Agent Registry wallet using native HTTPS and secure memory.

        Args:
            api_key: Agent Registry API key (masked wallet).
            sentinel_host: Base URL of the Sentinel host to fetch the wallet from.
            verify: Verify TLS certificates when fetching the wallet.
            require_lock: Fail if mlock cannot lock the secure page.
            harden_process: Disable core dumps process-wide before fetching the wallet.
            include_creds: Attach env-var AWS credentials to the fetch request.
                Set True only for external wallets stored under those creds.
        """

        if not harden_process:
            warnings.warn(
                "harden_process=False disables RLIMIT_CORE=0 and PR_SET_DUMPABLE=0. "
                "Secret key material may appear in core dumps. ",
                stacklevel=2,
            )
        elif harden_process:
            apply_hardening()

        account, (org_name, subgroup, purpose) = SecureAccount.from_api_key(
            api_key,
            sentinel_host,
            verify=verify,
            require_lock=require_lock,
            include_creds=include_creds,
        )
        w = cls()
        w._added_accounts = [account]
        w._current_account_index = SelectedAccount.External(0)
        w._secure_api_key = True
        if org_name:
            w._org = OrgInfo(
                name=org_name,
                subgroup=subgroup or None,
                purpose=purpose or "",
            )
        return w

    @classmethod
    def from_account_export_file(
        cls, path: Union[str, Path], **kwargs: Any
    ) -> "Wallet":
        """Backward-compatible alias for wallet.wc/account export files."""
        return cls.from_wallet_file(path, **kwargs)

    @classmethod
    def from_private_key_and_address(
        cls, private_key: PrivateKey, account_address: str
    ) -> "Wallet":
        """Create a Wallet from a raw private key and a pre-minted address."""
        secret_bytes = bytes.fromhex(private_key._hex)
        w = cls()
        w._derived_accounts = []
        w._added_accounts = [Account(secret_bytes, account_address)]
        w._current_account_index = SelectedAccount.External(0)
        return w

    @classmethod
    def from_secure_private_key_file(
        cls,
        path: Union[str, Path],
        account_address: str,
        *,
        require_lock: bool = True,
        harden_process: bool = True,
    ) -> "Wallet":
        """Load a raw hex key into the opt-in Linux secure-memory backend.

        Unlike :meth:`from_wallet_file`, this constructor does not read key
        contents into a Python ``str`` or ``bytes`` object. The file must contain
        exactly one 32-byte secp256k1 private key encoded as 64 hexadecimal
        characters (optional trailing whitespace is accepted).

        Args:
            path: File containing the hex private key.
            account_address: Sentinel-minted address associated with the key.
            require_lock: Fail if ``mlock`` cannot lock the secure page.
            harden_process: Also disable core dumps process-wide. This cannot be
                reversed, so it is opt-in.
        """
        from .secure_wallet import SecureAccount, harden_process as apply_hardening

        if harden_process:
            apply_hardening()
        account = SecureAccount.from_private_key_file(
            path, account_address, require_lock=require_lock
        )
        w = cls()
        w._added_accounts = [account]
        w._current_account_index = SelectedAccount.External(0)
        return w

    # ── Account management ────────────────────────────────────────────────────

    def set_index(self, selected: SelectedAccount) -> None:
        """Switch the active account.

        Raises:
            IndexError: If the index is out of bounds for the target list.
        """
        if selected.kind == "derived":
            if selected.index >= len(self._derived_accounts):
                raise IndexError(
                    f"derived account index {selected.index} out of bounds "
                    f"(have {len(self._derived_accounts)} derived account(s))"
                )
        elif selected.kind == "external":
            if selected.index >= len(self._added_accounts):
                raise IndexError(
                    f"external account index {selected.index} out of bounds "
                    f"(have {len(self._added_accounts)} external account(s))"
                )
        else:
            raise ValueError(f"unknown account kind: {selected.kind!r}")
        self._current_account_index = selected

    # ── Accessors ─────────────────────────────────────────────────────────────

    @property
    def secret_key(self) -> Secp256k1PrivateKey:
        """Return the current account's secp256k1 secret key. Handle with care."""
        return self._current_account().get_secret_key()

    def get_public_key(self) -> Secp256k1PublicKey:
        """Return the current account's secp256k1 public key."""
        return self._current_account().get_public_key()

    def get_address(self) -> str:
        """Return the current account's sentinel-minted 72-char hex address."""
        return self._current_account().get_address()

    def derived_account_count(self) -> int:
        """Return the number of BIP32 HD-derived accounts."""
        return len(self._derived_accounts)

    def external_account_count(self) -> int:
        """Return the number of externally added accounts."""
        return len(self._added_accounts)

    def current_account_index(self) -> SelectedAccount:
        """Return the currently selected account index."""
        return self._current_account_index

    @property
    def org(self) -> Optional[OrgInfo]:
        """Return the org info if this wallet has a linked organization."""
        return self._org

    def sign(self, buf: bytes) -> str:
        """Sign buf with the current account using ECDSA secp256k1.

        The message is hashed with SHA-256, then signed. Returns hex-encoded
        64-byte compact signature (r || s), matching the Rust libsecp256k1 format.
        """
        return self._current_account().sign(buf)

    def security_status(self) -> Optional[dict[str, bool]]:
        """Return native protection status, or ``None`` for a regular wallet."""
        account = self._current_account()
        status = getattr(account, "security_status", None)
        return status() if status is not None else None

    def close(self) -> None:
        """Wipe native key storage when this is a secure-memory wallet."""
        for account in (*self._derived_accounts, *self._added_accounts):
            close = getattr(account, "close", None)
            if close is not None:
                close()

    # ── Internal ──────────────────────────────────────────────────────────────

    def _current_account(self) -> Account:
        sel = self._current_account_index
        if sel.kind == "derived":
            return self._derived_accounts[sel.index]
        return self._added_accounts[sel.index]


# ── BIP32 helpers ─────────────────────────────────────────────────────────────

_SECP256K1_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141


def _decode_xprv(xprv_str: str) -> tuple[bytes, bytes]:
    """Decode a base58check xprv string and return (key_bytes_32, chain_code_32).

    xprv wire format (78 bytes after base58check decode):
      version(4) + depth(1) + fingerprint(4) + child_index(4)
      + chain_code(32) + key_prefix_0x00(1) + key(32)
    """
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
        data = b'\x00' + parent_key + struct.pack(">I", index + 0x80000000)
    else:
        compressed_pub = Secp256k1PrivateKey(parent_key).public_key.format(compressed=True)
        data = compressed_pub + struct.pack(">I", index)

    I = _hmac.new(parent_chain, data, hashlib.sha512).digest()
    IL, IR = I[:32], I[32:]
    child_int = (int.from_bytes(IL, 'big') + int.from_bytes(parent_key, 'big')) % _SECP256K1_N
    return child_int.to_bytes(32, 'big'), IR


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
    child_key, _ = _bip32_derive_child(master_key, master_chain, first["index"], hardened=False)
    pk_hex = Secp256k1PrivateKey(child_key).public_key.format(compressed=True).hex()

    if pk_hex == first["public_key"]:
        return master_key, master_chain

    # Root xprv — traverse m/44'/9345'/0'/0
    key, chain = master_key, master_chain
    for idx, hardened in [(44, True), (9345, True), (0, True), (0, False)]:
        key, chain = _bip32_derive_child(key, chain, idx, hardened=hardened)
    return key, chain


# ── Signature helpers ─────────────────────────────────────────────────────────

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
