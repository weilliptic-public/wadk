"""Transaction primitives: headers, status, results, canonical JSON."""

from __future__ import annotations

import uuid
from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional

from .utils import current_time_millis


class TransactionStatus(str, Enum):
    """Lifecycle states for a transaction."""

    IN_PROGRESS = "InProgress"
    CONFIRMED = "Confirmed"
    FINALIZED = "Finalized"
    FAILED = "Failed"


@dataclass
class OrgContext:
    """The organization a transaction is signed under.

    Carried in the signed header and surfaced to applets as ``Runtime::org()``,
    so an applet can scope itself to the caller's org without taking one as a
    method argument.

    The wire shape is load-bearing: it must serialize exactly as the node's
    ``OrgContext`` does -- keys ``org``/``subgroup``, with ``subgroup: null``
    when absent rather than omitted. The signature is a SHA-256 over key-sorted
    JSON that both sides rebuild independently, so a renamed or dropped key
    changes the digest and the node rejects the transaction.
    """

    org: str
    subgroup: Optional[str] = None

    def to_dict(self) -> dict[str, Any]:
        """Digest/wire representation. ``subgroup`` is always present."""
        return {"org": self.org, "subgroup": self.subgroup}


@dataclass
class TransactionHeader:
    """Immutable transaction header (except optional signature)."""

    nonce: int
    public_key: str
    from_addr: str
    to_addr: str
    signature: Optional[str] = None
    weilpod_counter: int = 0
    creation_time: int = 0
    # Random UUIDv4. Covered by the signature (see _sign_execute_args) and
    # mixed into the node's get_txn_id(), so two transactions never collide
    # on id even if nonce happens to match.
    salt: str = ""
    # Organization the signing wallet is acting under, or None when it has no
    # active org. Covered by the signature (see _sign_execute_args).
    org: Optional[OrgContext] = None

    def __post_init__(self) -> None:
        if self.creation_time == 0:
            self.creation_time = int(current_time_millis())
        if not self.salt:
            self.salt = str(uuid.uuid4())

    def set_signature(self, signature: str) -> None:
        """Attach a hex-encoded signature to the header."""
        self.signature = signature

    def parsed_public_key_bytes(self) -> bytes:
        """Decode the hex public_key to bytes (full/uncompressed 65 bytes expected)."""
        return bytes.fromhex(self.public_key)


@dataclass
class BaseTransaction:
    """Submission-ready transaction bundle: header + TTL."""

    header: TransactionHeader


@dataclass
class TransactionResult:
    """Canonical result envelope returned by the chain for a submitted transaction."""

    status: TransactionStatus = TransactionStatus.IN_PROGRESS
    block_height: int = 0
    batch_id: str = ""
    batch_author: str = ""
    tx_idx: int = 0
    txn_result: str = ""
    creation_time: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TransactionResult":
        """Build from API response dict."""
        status_str = data.get("status", "InProgress")
        try:
            status = TransactionStatus(status_str)
        except ValueError:
            status = TransactionStatus.IN_PROGRESS
        return cls(
            status=status,
            block_height=int(data.get("block_height", 0)),
            batch_id=data.get("batch_id", ""),
            batch_author=data.get("batch_author", ""),
            tx_idx=int(data.get("tx_idx", 0)),
            txn_result=data.get("txn_result", ""),
            creation_time=data.get("creation_time", ""),
        )
