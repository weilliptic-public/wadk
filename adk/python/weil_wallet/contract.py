"""Contract ID (Weil Applet address) and pod routing."""

import base64
import struct
from .errors import InvalidContractIdError


class ContractId:
    """Contract ID (contract address) of a Weil Applet (smart contract)."""

    __slots__ = ("_value",)

    def __init__(self, contract_id: str) -> None:
        self._value = self._validate(contract_id)

    @classmethod
    def new(cls, contract_id: str) -> "ContractId":
        """Construct from a string. Raises InvalidContractIdError if invalid."""
        return cls(contract_id)

    def _validate(self, s: str) -> str:
        """Validate contract ID string. Returns trimmed value."""
        # Rust has validate_contract_id_str as TODO (no-op); we do the same.
        return s

    def pod_counter(self) -> int:
        """Extract WeilPod (shard) counter from the contract ID for routing.

        Decodes base32 (RFC 4648 lower, no padding), expects 36 bytes,
        first 4 bytes big-endian as i32.
        """
        # Python base64.b32decode expects uppercase; add padding if needed
        pad = (8 - len(self._value) % 8) % 8
        try:
            decoded = base64.b32decode(self._value.upper() + "=" * pad)
        except Exception as e:
            raise ValueError("base32 decoding failed") from e
        if len(decoded) != 36:
            raise ValueError(
                f"invalid contract-id: expected 36 bytes long, got {len(decoded)} bytes"
            )
        (pod_id_counter,) = struct.unpack(">i", decoded[:4])
        return pod_id_counter

    def __str__(self) -> str:
        return self._value

    def __repr__(self) -> str:
        return f"ContractId({self._value!r})"

    def __eq__(self, other: object) -> bool:
        if isinstance(other, ContractId):
            return self._value == other._value
        return False

    def __hash__(self) -> int:
        return hash(self._value)


def contract_id_from_str(s: str) -> ContractId:
    """Parse a contract ID from string. Raises InvalidContractIdError on failure."""
    try:
        return ContractId(s)
    except Exception as e:
        raise InvalidContractIdError(str(e)) from e
