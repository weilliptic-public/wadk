import hashlib
import time
import coincurve

MAX_TIMESTAMP_AGE_SECONDS = 60


def _derive_address(uncompressed_pubkey: bytes) -> str:
    """
    Mirror of TxnUtils::get_address_from_public_key (transaction.rs:287).

    address = hex( SHA256( uncompressed_65_byte_secp256k1_pubkey ) )
    """
    return hashlib.sha256(uncompressed_pubkey).hexdigest()


def verify_weil_signature(
    wallet_address: str,
    signature_hex: str,
    message: str,
    timestamp: str,
    max_age_seconds: int = MAX_TIMESTAMP_AGE_SECONDS,
) -> bool:
    """
    Verify the four auth headers emitted by get_auth_headers() in mcp_client.py.

    Steps (mirrors verify_me in transaction.rs):
      1. Reject stale timestamps (anti-replay).
      2. Compute SHA256 of the raw message bytes — this is the digest that
         weil_wallet.sign() signs over.
      3. Decode the 64-byte compact signature (r || s).
      4. Recover the secp256k1 public key from (signature, digest).
         The compact format carries no recovery-id, so we try 0 and 1.
      5. Derive the address from the recovered key and compare with
         X-Wallet-Address.

    Args:
        wallet_address: X-Wallet-Address header value
        signature_hex:  X-Signature header value  (hex of 64 compact bytes)
        message:        X-Message header value     (the JSON string that was signed)
        timestamp:      X-Timestamp header value   (Unix seconds, as string)

    Returns:
        True if the signature is valid and the address matches.
    """
    # 1. Timestamp freshness
    try:
        ts = int(timestamp)
    except (ValueError, TypeError):
        return False
    if abs(int(time.time()) - ts) > max_age_seconds:
        return False

    # 2. SHA256 of the message — mirrors hash_sha256(verify_payload.as_bytes())
    digest = hashlib.sha256(message.encode("utf-8")).digest()  # 32 bytes

    # 3. Decode compact 64-byte signature (r || s)
    try:
        sig_bytes = bytes.fromhex(signature_hex)
    except ValueError:
        return False
    if len(sig_bytes) != 64:
        return False

    # 4 + 5. Recover public key and check address.
    # coincurve expects a 65-byte recoverable signature: r(32) || s(32) || v(1)
    # v is the recovery id. We try 0 and 1 (2/3 are valid only for edge-case keys).
    for recovery_id in (0, 1):
        try:
            recoverable_sig = sig_bytes + bytes([recovery_id])
            pub = coincurve.PublicKey.from_signature_and_message(
                recoverable_sig,
                digest,
                hasher=None,  # digest is already SHA256-hashed; don't hash again
            )
            # format(compressed=False) → 65 bytes (0x04 || x || y)
            # mirrors libsecp256k1 PublicKey::serialize() which returns uncompressed form
            derived = _derive_address(pub.format(compressed=False))
            if derived == wallet_address.lower():
                return True
        except Exception:
            continue

    return False


wallet_address = "1de829ac665627a8f43b455b87c7ec33a19db63d2a8c7ae933a47de741d102b1"
signature = "725af37f8e96c6ffd29d22580cae1ff727470a6a7622999d903a28b31c22d898793c5df02f16b15ec554dda7cd54270953060ea076c3cccf157580013066103d"
message = '{"timestamp":"1772113644"}'
timestamp = "1772113644"

flag = verify_weil_signature(wallet_address, signature, message, timestamp)

print(flag)
