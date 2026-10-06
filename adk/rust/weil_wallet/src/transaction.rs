//! # Transaction primitives (headers, status, results, utils)
//!
//! This module defines core transaction data structures used by the client/SDK:
//!
//! - [`TransactionHeader`]: immutable header fields plus a mutable optional signature.
//! - [`TransactionStatus`]: lifecycle states for a transaction.
//! - [`TransactionResult`]: canonical result envelope returned from the chain.
//! - [`BaseTransaction`]: header + TTL wrapper suitable for submission.
//! - [`value_to_btreemap`]: helper to normalize a `serde_json::Value::Object` into a `BTreeMap`.
//!
//! Timestamps are recorded in **milliseconds since UNIX epoch**. Addresses are stored as
//! `Arc<String>` for cheap cloning. Public keys are expected to be **hex-encoded**.

use crate::current_time_millis;
use libsecp256k1::{PublicKey, PublicKeyFormat};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

/// The organization context a transaction is signed under.
///
/// Carried in the signed header and surfaced to applets as `Runtime::org()`,
/// so an applet can scope itself to the caller's org without taking one as a
/// method argument.
///
/// # Wire shape is load-bearing
///
/// This must serialize *exactly* as the node's `OrgContext` does — field names
/// `org`/`subgroup`, and `subgroup: null` when absent rather than omitted.
/// The signature is a SHA-256 over key-sorted JSON that both sides rebuild
/// independently, so a skipped field or a renamed key changes the digest and
/// the node rejects the transaction with "Verification failed".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrgContext {
    /// Organization identity name, e.g. `"weil"`.
    pub org: String,
    /// Subgroup within the organization, e.g. `"engg"`. `None` at org level.
    /// Deliberately **not** `skip_serializing_if` — see the note above.
    pub subgroup: Option<String>,
}

impl From<&crate::wallet::OrgInfo> for OrgContext {
    /// Maps the wallet-file view of a membership onto the wire shape the node
    /// expects. `purpose` is intentionally dropped — it is advisory metadata
    /// resolved from the Identity contract at runtime, not part of the claim.
    fn from(info: &crate::wallet::OrgInfo) -> Self {
        OrgContext {
            org: info.name.clone(),
            // Normalize `Some("")` to `None`: an empty subgroup means
            // org-level, and the two must not produce different digests.
            subgroup: info.subgroup.clone().filter(|s| !s.is_empty()),
        }
    }
}

/// Immutable transaction header (except for the optional signature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TransactionHeader {
    /// Client-controlled, monotonically increasing number to prevent replays.
    pub nonce: usize,
    /// Hex-encoded public key (secp256k1).
    pub public_key: String,
    /// Sender address (shared, cheap to clone).
    pub from_addr: Arc<String>,
    /// Recipient contract/address (shared, cheap to clone).
    pub to_addr: Arc<String>,
    /// Optional hex-encoded ECDSA signature over the canonical payload.
    pub signature: Option<String>,
    /// Target WeilPod (shard) counter used for routing.
    pub weilpod_counter: i32,
    /// Creation timestamp in **ms** since UNIX epoch.
    pub creation_time: u64,
    /// Random UUIDv4. Covered by the signature (see `sign_execute_args`) and
    /// mixed into the node's `get_txn_id()`, so two transactions never
    /// collide on id even if nonce happens to match.
    pub salt: String,
    /// Organization the signing wallet is acting under, or `None` when it has
    /// no active org. Covered by the signature (see `execute_payload`).
    ///
    /// `serde(default)` keeps payloads from older nodes/clients — which omit
    /// the key entirely — deserializable.
    #[serde(default)]
    pub org: Option<OrgContext>,
}

impl TransactionHeader {
    /// Construct a new header; `signature` starts as `None`.
    ///
    /// `creation_time` is set to the current `current_time_millis()` value.
    pub fn new(
        nonce: usize,
        public_key: String,
        from_addr: Arc<String>,
        to_addr: Arc<String>,
        weilpod_counter: i32,
    ) -> Self {
        Self {
            nonce,
            public_key,
            from_addr,
            to_addr,
            signature: None,
            weilpod_counter,
            creation_time: current_time_millis() as u64,
            salt: uuid::Uuid::new_v4().to_string(),
            org: None,
        }
    }

    /// Sets the org context. Must be called **before** signing — org is part
    /// of the signed digest, so changing it afterwards invalidates the
    /// signature.
    pub fn set_org(&mut self, org: Option<OrgContext>) {
        self.org = org
    }

    /// Attach a hex-encoded signature to the header.
    pub fn set_signature(&mut self, signature: &str) {
        self.signature = Some(String::from(signature))
    }

    /// Parse the hex-encoded `public_key` into a secp256k1 [`PublicKey`].
    ///
    /// Expects **full (uncompressed)** format bytes.
    ///
    /// # Errors
    /// - If the hex string cannot be decoded.
    /// - If the resulting bytes are not a valid secp256k1 public key.
    pub fn parsed_public_key(&self) -> anyhow::Result<PublicKey> {
        let public_key_as_vec = hex::decode(self.public_key.as_bytes()).unwrap();
        let public_key =
            PublicKey::parse_slice(public_key_as_vec.as_slice(), Some(PublicKeyFormat::Full))?;

        Ok(public_key)
    }
}

/// Lifecycle states for a transaction.
#[derive(PartialEq, Eq, Clone, Deserialize, Serialize, Debug)]
pub enum TransactionStatus {
    /// Transaction is submitted and awaiting confirmation.
    InProgress,
    /// Transaction has been included/confirmed in a block.
    Confirmed,
    /// Transaction is finalized/irreversible by the consensus rules.
    Finalized,
    /// Transaction failed (rejected, reverted, or invalid).
    Failed,
}

impl Default for TransactionStatus {
    fn default() -> Self {
        TransactionStatus::InProgress
    }
}

/// Canonical result envelope returned by the chain for a submitted transaction.
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct TransactionResult {
    /// Final status (see [`TransactionStatus`]).
    pub status: TransactionStatus,
    /// Block height where the transaction was processed (if applicable).
    pub block_height: u64,
    /// Batch identifier grouping multiple transactions.
    pub batch_id: String,
    /// Author/producer of the batch (e.g., block proposer).
    pub batch_author: String,
    /// Index of the transaction within the batch.
    pub tx_idx: usize,
    /// Application-level JSON result serialized as a string.
    pub txn_result: String,
    /// Creation timestamp (RFC3339 or ms-string; depends on producer).
    pub creation_time: String,
}

/// Submission-ready transaction bundle: header + TTL.
pub(crate) struct BaseTransaction {
    /// Immutable/mutable header (signature may be added later).
    pub header: TransactionHeader,
}

impl BaseTransaction {
    /// Create a new base transaction with the default TTL (`DEFAULT_TRANSACTION_TTL`).
    pub fn new(header: TransactionHeader) -> Self {
        BaseTransaction { header }
    }
}

/// Convert a `serde_json::Value::Object` into a `BTreeMap<String, Value>`.
///
/// Non-object values produce an empty map.
pub fn value_to_btreemap(value: Value) -> BTreeMap<String, Value> {
    let mut btree_map = BTreeMap::new();

    if let Value::Object(map) = value {
        for (key, value) in map {
            btree_map.insert(key, value);
        }
    }

    btree_map
}
