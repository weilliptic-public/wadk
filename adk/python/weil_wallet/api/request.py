"""Request types for the platform API (submit transaction)."""

from dataclasses import dataclass
from typing import Any, Optional

from ..contract import ContractId
from ..transaction import TransactionHeader


@dataclass
class Verifier:
    """Signature verifier strategy tag sent with every transaction."""

    ty: str = "DefaultVerifier"


@dataclass
class UserTransaction:
    """The user-visible part of a transaction: which contract method to call and with what args."""

    ty: str = "SmartContractExecutor"
    contract_address: Optional[ContractId] = None
    contract_method: str = ""
    contract_input_bytes: Optional[str] = None
    should_hide_args: bool = True


@dataclass
class Transaction:
    """Full transaction envelope: header, verifier, and user payload."""

    is_xpod: bool = False
    txn_header: Optional[TransactionHeader] = None
    verifier: Optional[Verifier] = None
    user_txn: Optional[UserTransaction] = None


@dataclass
class SubmitTxnRequest:
    """Top-level request wrapper submitted to the platform API."""

    transaction: Optional[Transaction] = None

    def to_payload_dict(self) -> dict[str, Any]:
        """Build the JSON-serializable payload (with 'type' for serde compatibility)."""
        txn = self.transaction
        if not txn or not txn.txn_header or not txn.verifier or not txn.user_txn:
            raise ValueError("incomplete transaction")
        h = txn.txn_header
        return {
            "transaction": {
                "is_xpod": txn.is_xpod,
                "txn_header": {
                    "nonce": h.nonce,
                    "public_key": h.public_key,
                    "from_addr": h.from_addr,
                    "to_addr": h.to_addr,
                    "signature": h.signature,
                    "weilpod_counter": h.weilpod_counter,
                    "creation_time": h.creation_time,
                },
                "verifier": {"type": txn.verifier.ty},
                "user_txn": {
                    "type": txn.user_txn.ty,
                    "contract_address": str(txn.user_txn.contract_address),
                    "contract_method": txn.user_txn.contract_method,
                    "contract_input_bytes": txn.user_txn.contract_input_bytes,
                    "should_hide_args": txn.user_txn.should_hide_args,
                },
            }
        }
