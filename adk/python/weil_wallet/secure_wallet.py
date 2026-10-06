"""Opt-in Linux secure-memory wallet support.

The private key file is opened as a file descriptor and parsed by the native
extension. Private-key bytes are never returned to Python.
"""

from __future__ import annotations

import hashlib
import os
import stat
import sys
from pathlib import Path
from typing import Any, Union

from coincurve import PublicKey as Secp256k1PublicKey


def _native() -> Any:
    if not sys.platform.startswith("linux"):
        raise RuntimeError("secure-memory wallets are currently supported on Linux only")
    try:
        from . import _secure_wallet
    except ImportError as exc:
        raise RuntimeError(
            "secure-memory support is not built; reinstall weil-wallet from source "
            "on Linux with a C compiler and OpenSSL development headers"
        ) from exc
    return _secure_wallet


class SecureAccount:
    """An account whose private key is owned by opaque native secure memory.

    For masked API-key wallets the handle retains the masked wallet JSON (no
    local key); the public key comes from the JSON's stored compressed hex.
    """

    __slots__ = ("_native_key", "_public_key", "_address")

    def __init__(
        self,
        native_key: Any,
        address: str,
        public_key_hex: str | None = None,
    ) -> None:
        """Wrap a native secure-memory key handle.

        Args:
            native_key: Opaque native key handle.
            address: Sentinel-minted account address.
            public_key_hex: Hex public key for masked API-key handles, which
                hold no local secret key.
        """
        self._native_key = native_key
        self._address = address
        if public_key_hex is not None:
            # Masked API-key handle: no local key; pubkey parsed from JSON hex.
            self._public_key = Secp256k1PublicKey(bytes.fromhex(public_key_hex))
        else:
            # File/key handle: derive the public key from the native secret.
            self._public_key = Secp256k1PublicKey(native_key.public_key())

    @classmethod
    def from_private_key_file(
        cls,
        path: Union[str, Path],
        address: str,
        *,
        require_lock: bool = True,
    ) -> "SecureAccount":
        """Load a 64-character hex key without materializing it in Python."""
        flags = os.O_RDONLY
        if hasattr(os, "O_CLOEXEC"):
            flags |= os.O_CLOEXEC
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        fd = os.open(os.fspath(path), flags)
        try:
            metadata = os.fstat(fd)
            if not stat.S_ISREG(metadata.st_mode):
                raise ValueError("secure key path must refer to a regular file")
            if metadata.st_mode & 0o077:
                raise PermissionError(
                    "secure key file must not be accessible by group or other users; "
                    "use chmod 600"
                )
            key = _native().SecureKey.from_fd(fd, require_lock=require_lock)
        finally:
            os.close(fd)
        return cls(key, address)

    @classmethod
    def from_wallet_file(
        cls,
        path: Union[str, Path],
        *,
        require_lock: bool = True,
    ) -> tuple["SecureAccount", tuple[str | None, str | None, str | None]]:
        """Parse a full wallet file natively and expose only public metadata."""
        flags = os.O_RDONLY
        if hasattr(os, "O_CLOEXEC"):
            flags |= os.O_CLOEXEC
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        fd = os.open(os.fspath(path), flags)
        try:
            metadata = os.fstat(fd)
            if not stat.S_ISREG(metadata.st_mode):
                raise ValueError("secure wallet path must refer to a regular file")
            if metadata.st_mode & 0o077:
                raise PermissionError(
                    "secure wallet file must not be accessible by group or other users; "
                    "use chmod 600"
                )
            key, address, org, subgroup, purpose = _native().SecureKey.from_wallet_fd(
                fd, require_lock=require_lock
            )
        finally:
            os.close(fd)
        return cls(key, address), (org, subgroup, purpose)

    @classmethod
    def from_api_key(
        cls,
        api_key: str,
        sentinel_host: str,
        *,
        verify: bool = True,
        require_lock: bool = True,
        include_creds: bool = False,
    ) -> tuple["SecureAccount", tuple[str | None, str | None, str | None]]:
        """Fetch an agent wallet over native HTTPS and retain it in secure memory.

        The wallet response is received into mlocked memory and never exposed to
        HTTPX or Python strings. The wallet is fetched with ``unmasked: true``,
        so the secret key is derived in secure memory and signing is done locally
        via the native backend.

        Args:
            include_creds: Attach env-var AWS credentials to the fetch request.
                Set True only for external wallets stored under those creds.
        """
        endpoint = sentinel_host.rstrip("/") + "/get_agent_wallet"
        key, address, org, subgroup, purpose = _native().SecureKey.from_api(
            api_key,
            endpoint,
            verify=verify,
            require_lock=require_lock,
            include_creds=include_creds,
        )
        if include_creds:
            for _k in ("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY",
                        "AWS_REGION", "AWS_BUCKET_NAME"):
                os.environ.pop(_k, None)
        return cls(key, address), (org, subgroup, purpose)

    def get_public_key(self) -> Secp256k1PublicKey:
        """Return the account's secp256k1 public key."""
        return self._public_key

    def get_address(self) -> str:
        """Return the sentinel-minted 72-char hex address."""
        return self._address

    def get_secret_key(self) -> None:
        """Always ``None`` — the secret key never leaves native secure memory."""
        return None

    def sign(self, buf: bytes) -> str:
        """Sign ``buf`` inside native secure memory.

        SHA-256 hashes the message; returns a hex-encoded 64-byte compact
        ECDSA secp256k1 signature.
        """
        digest = hashlib.sha256(buf).digest()
        return self._native_key.sign_digest(digest).hex()

    def security_status(self) -> dict[str, bool]:
        """Return native protection status (mlock, dump-exclusion, etc.)."""
        return self._native_key.security_status()

    def close(self) -> None:
        """Wipe and unmap the native secure-memory page."""
        self._native_key.close()


def harden_process() -> None:
    """Disable core dumps for this process using RLIMIT_CORE and PR_SET_DUMPABLE."""
    _native().harden_process()

