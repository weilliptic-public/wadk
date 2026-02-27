//! # Cryptographic & utility helpers
//!
//! Small, self-contained utilities used across the codebase:
//! - `hash_sha256` — compute SHA-256 over a byte slice
//! - `get_address_from_public_key` — derive an address (hex SHA-256 of compressed secp256k1 pubkey)
//! - `timestamp` / `current_time_millis` — Unix epoch time helpers
//! - `compress` — JSON-serialize a value and GZIP-compress it
//!
//! These helpers are `pub(crate)` and intended for internal use within the crate.

use flate2::{write::GzEncoder, Compression};
use libsecp256k1::PublicKey;
use openssl::sha::Sha256;
use serde::Serialize;
use std::io::Write;
use std::{sync::Arc, time::SystemTime};

/// Compute the SHA-256 digest of `buf`.
///
/// # Returns
/// A 32-byte array containing the hash.
pub(crate) fn hash_sha256(buf: &[u8]) -> [u8; 32] {
    let mut sha256 = Sha256::new();
    sha256.update(buf);
    sha256.finish()
}

/// Derive an address string from a secp256k1 [`PublicKey`].
///
/// The address is defined as the **hex-encoded SHA-256** of the key's **compressed** bytes,
/// wrapped in an [`Arc<String>`] for cheap cloning.
///
/// # Notes
/// - Uses `libsecp256k1::PublicKey::serialize()` (compressed form).
/// - Not a standard (e.g., not ETH keccak-160), but a crate-local convention.
pub(crate) fn get_address_from_public_key(public_key: &PublicKey) -> Arc<String> {
    let addr = hash_sha256(&public_key.serialize());
    let addr = hex::encode(&addr);

    Arc::new(addr)
}

/// Return the current Unix time in **milliseconds** as `f64`.
///
/// Convenience wrapper over [`timestamp`] for float-compatible call sites.
pub(crate) fn current_time_millis() -> f64 {
    timestamp() as f64
}

/// Return the current Unix time in **milliseconds** as `u128`.
///
/// Uses [`SystemTime::now`] and `duration_since(UNIX_EPOCH)`.
/// Panics only if system time is before the Unix epoch (unlikely on sane systems).
pub(crate) fn timestamp() -> u128 {
    let now = SystemTime::now();
    let elapsed = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    elapsed.as_millis()
}

/// Serialize `value` to JSON and **GZIP-compress** the bytes.
///
/// # Returns
/// A `Vec<u8>` containing the compressed payload.
///
/// # Errors
/// - Propagates JSON serialization errors from `serde_json::to_string`.
/// - Propagates I/O errors from the gzip encoder.
pub(crate) fn compress<T: Serialize>(value: &T) -> Result<Vec<u8>, anyhow::Error> {
    let json_str = serde_json::to_string(value)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(json_str.as_bytes())?;

    Ok(encoder.finish()?)
}
