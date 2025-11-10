//! # WRC-734-style Key Management
//!
//! This module implements a lightweight, on-chain key management pattern inspired by
//! ERC-734 / identity controller contracts. It defines:
//!
//! - [`KeyPurpose`]: roles a key can have (Management / Execution / Claim / Encryption / Empty)
//! - [`WRC734`]: trait describing the expected key-management interface
//! - [`KeyManager`]: a concrete implementation backed by [`WeilMap`]s
//!
//! Keys are stored as strings (e.g., hex-encoded public keys or address identifiers)
//! and mapped to a set of purposes. Reverse indexes are maintained to query keys by purpose.
//!
//! ## Data structures
//! - `keys: WeilMap<String, BTreeSet<KeyPurpose>>` — key → set of purposes
//! - `keys_by_purpose: WeilMap<KeyPurpose, BTreeSet<String>>` — purpose → set of keys
//! - `executions: WeilMap<u128, Execution>` — queued/executed operations (placeholder here)
//!
//! ## Notes
//! - `execute` is left as `todo!()` for now; it typically enforces approvals and triggers
//!   value/data dispatch.
//! - `KeyType` is captured but not yet enforced in this implementation; retained for spec parity.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use weil_macros::WeilType;
use weil_rs::collections::{map::WeilMap, WeilIdGenerator};

/// 32-byte blob alias, often used for hashes or fixed public key digests.
pub type Bytes32 = [u8; 32];

/// Role/purpose that a key can be granted within the identity/agent.
#[derive(Serialize, Deserialize, WeilType, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum KeyPurpose {
    /// Admin/owner-level authority (can manage keys/policies).
    Management,
    /// Authority to initiate/approve **executions** (transactions, calls).
    Execution,
    /// Authority to make **claims** (attestations/credentials).
    Claim,
    /// Authority for **encryption** / key agreement.
    Encryption,
    /// Empty/placeholder purpose.
    Empty,
}

impl KeyPurpose {
    /// Parse a [`KeyPurpose`] from its string variant.
    ///
    /// Accepts: `"Management" | "Execution" | "Claim" | "Encryption" | "Empty"`.
    ///
    /// # Errors
    /// Returns `"invalid purpose"` if the input is not a known variant.
    pub fn from_str(purpose: String) -> Result<KeyPurpose, String> {
        let purpose = match purpose.as_str() {
            "Management" => KeyPurpose::Management,
            "Execution" => KeyPurpose::Execution,
            "Claim" => KeyPurpose::Claim,
            "Encryption" => KeyPurpose::Encryption,
            "Empty" => KeyPurpose::Empty,
            _ => return Err("invalid purpose".to_string()),
        };

        Ok(purpose)
    }
}

/// Numeric key type code.
///
/// Examples: `1 = ECDSA`, `2 = RSA`, etc. (Captured for spec parity; not enforced here.)
pub type KeyType = u32;

/// String alias for an address/identifier (wallet, contract, etc.).
pub type Address = String;

/// Composite identity of a key entry: (purpose, type).
///
/// Used to track purpose assignments; the actual key material is the map key (`String`).
#[derive(Serialize, Deserialize, WeilType, Hash, PartialEq, PartialOrd, Eq, Ord)]
struct Key {
    /// Granted purpose for this key.
    purpose: KeyPurpose,
    /// Key type (e.g., 1 = ECDSA, 2 = RSA).
    key_type: KeyType, // e.g. 1 = ECDSA, 2 = RSA, etc.
}

/// Execution intent/record for value and data dispatch.
///
/// In a full implementation, this would be queued, approved, and executed according to policy.
#[derive(Serialize, Deserialize, WeilType)]
struct Execution {
    /// Destination address.
    to: Address,
    /// Transferred value (token units, chain-native amount, etc.).
    value: u128,
    /// Calldata / payload to execute.
    data: Vec<u8>,
    /// Whether this execution has acquired sufficient approvals.
    approved: bool,
    /// Whether this execution has been finalized/executed.
    executed: bool,
}

/// Trait describing the WRC-734 key management surface.
///
/// Implementations manage key→purpose mappings and provide convenience methods to
/// query, add, remove keys, and to initiate executions.
pub trait WRC734 {
    /// Return all purposes currently granted to `key`.
    fn get_key(&self, key: String) -> Option<BTreeSet<KeyPurpose>>;

    /// Check whether `key` has the specified `purpose`.
    fn key_has_purpose(&self, key: String, purpose: KeyPurpose) -> bool;

    /// Return all keys that currently hold a given `purpose`.
    fn get_keys_by_purpose(&self, purpose: KeyPurpose) -> Vec<String>;

    /// Add (or extend) a `key` with a `purpose` and `key_type`.
    ///
    /// Idempotent if the mapping already exists.
    fn add_key(
        &mut self,
        key: String,
        purpose: KeyPurpose,
        key_type: KeyType,
    ) -> Result<(), anyhow::Error>;

    /// Batch-add multiple `(key, purpose, key_type)` entries.
    ///
    /// Returns the **first** error encountered (if any).
    fn add_keys(&mut self, keys: Vec<(String, KeyPurpose, KeyType)>) -> Result<(), anyhow::Error>;

    /// Remove a `purpose` grant from `key`.
    ///
    /// Returns `true` on success (even if the mapping did not exist previously).
    fn remove_key(&mut self, key: String, purpose: KeyPurpose) -> Result<bool, anyhow::Error>;

    /// Create/record an execution intent (transfer/call). Returns an execution ID.
    fn execute(&mut self, to: Address, value: u128, data: Vec<u8>) -> Result<u128, anyhow::Error>;
}

/// Concrete key manager implementing [`WRC734`].
#[derive(Serialize, Deserialize, WeilType)]
pub struct KeyManager {
    /// Next execution identifier (monotonic counter).
    execution_id: u128,
    /// key → set(purposes)
    keys: WeilMap<String, BTreeSet<KeyPurpose>>,
    /// purpose → set(keys)
    keys_by_purpose: WeilMap<KeyPurpose, BTreeSet<String>>,
    /// id → execution record
    executions: WeilMap<u128, Execution>,
}

impl KeyManager {
    /// Construct a new [`KeyManager`], allocating internal maps via the provided ID generator.
    pub fn new(id_gen: &mut WeilIdGenerator) -> Self {
        Self {
            execution_id: 1,
            keys: WeilMap::new(id_gen.next_id()),
            keys_by_purpose: WeilMap::new(id_gen.next_id()),
            executions: WeilMap::new(id_gen.next_id()),
        }
    }
}

impl WRC734 for KeyManager {
    /// Get the set of purposes associated with `key`.
    fn get_key(&self, key: String) -> Option<BTreeSet<KeyPurpose>> {
        if let Some(key_entry) = self.keys.get(&key) {
            return Some(key_entry);
        } else {
            return None;
        }
    }

    /// Return whether `key` has a specific `purpose`.
    fn key_has_purpose(&self, key: String, purpose: KeyPurpose) -> bool {
        if let Some(key_entry) = self.keys.get(&key) {
            return key_entry.contains(&purpose);
        } else {
            return false;
        }
    }

    /// List all keys that have been granted `purpose`.
    fn get_keys_by_purpose(&self, purpose: KeyPurpose) -> Vec<String> {
        if let Some(keys) = self.keys_by_purpose.get(&purpose) {
            return keys.into_iter().collect();
        } else {
            Vec::new()
        }
    }

    /// Add (or extend) a mapping `key → purpose` and maintain the reverse index.
    ///
    /// Idempotent: inserting an already-present purpose is a no-op.
    fn add_key(
        &mut self,
        key: String,
        purpose: KeyPurpose,
        key_type: KeyType,
    ) -> Result<(), anyhow::Error> {
        if let Some(mut key_entry) = self.keys.get(&key) {
            if key_entry.insert(purpose) {
                self.keys.insert(key.clone(), key_entry);
            }
        } else {
            self.keys
                .insert(key.clone(), BTreeSet::from([purpose.clone()]));
        }

        if let Some(mut keys) = self.keys_by_purpose.get(&purpose) {
            if keys.insert(key.clone()) {
                self.keys_by_purpose.insert(purpose, keys);
            }
        } else {
            self.keys_by_purpose
                .insert(purpose, BTreeSet::from([key.clone()]));
        }

        Ok(())
    }

    /// Batch version of [`Self::add_key`]. Returns the first error if any insertion fails.
    fn add_keys(&mut self, keys: Vec<(String, KeyPurpose, KeyType)>) -> Result<(), anyhow::Error> {
        let mut errs = vec![];

        for (key, purpose, key_type) in keys {
            if let Err(err) = self.add_key(key, purpose, key_type) {
                errs.push(err);
            }
        }

        if errs.len() != 0 {
            return Err(anyhow::Error::msg(format!("{}", errs[0].to_string())));
        }

        Ok(())
    }

    /// Remove a `purpose` grant from `key` and maintain the reverse index.
    ///
    /// Always returns `Ok(true)`; absence is treated as a no-op.
    fn remove_key(&mut self, key: String, purpose: KeyPurpose) -> Result<bool, anyhow::Error> {
        if let Some(mut key_entry) = self.keys.get(&key) {
            key_entry.remove(&purpose);
            self.keys.insert(key.clone(), key_entry);
        }

        if let Some(mut keys) = self.keys_by_purpose.get(&purpose) {
            keys.remove(&key);
            self.keys_by_purpose.insert(purpose, keys);
        }

        Ok(true)
    }

    /// Queue/record an execution intent. Returns a unique execution ID.
    ///
    /// **Note:** This is currently unimplemented and will be wired to approval/dispatch logic.
    fn execute(&mut self, to: Address, value: u128, data: Vec<u8>) -> Result<u128, anyhow::Error> {
        todo!()
    }
}
