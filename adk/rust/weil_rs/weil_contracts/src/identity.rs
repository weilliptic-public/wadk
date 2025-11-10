//! # Identity token (key-managed identity data)
//!
//! This module defines a minimal identity data container backed by a [`WeilMap`],
//! plus helper flows to integrate with a **Key Manager** contract (WRC-734 style).
//!
//! - [`Identity`] trait: tiny K/V interface for identity-scoped metadata.
//! - [`IdentityToken`]: concrete implementation storing string K/V pairs.
//! - [`IdentityToken::init`]: seeds the key manager with management keys and
//!   records its address under `"key_management_addr"`.
//! - [`IdentityToken::add_key`]: permissioned call that requires the **sender**
//!   to have `KeyPurpose::Management` in the referenced key manager before it
//!   can add another key.

use crate::key_management::{KeyPurpose, KeyType};
use serde::{Deserialize, Serialize};
use weil_macros::WeilType;
use weil_rs::{
    collections::{map::WeilMap, WeilIdGenerator},
    runtime::Runtime,
};

/// Minimal identity K/V behavior.
///
/// Implementors store arbitrary string values under string keys.
pub trait Identity {
    /// Get a string value for `key`, if present.
    fn get_data(&self, key: &String) -> Option<String>;

    /// Set (insert or overwrite) a string `val` at `key`.
    fn set_data(&mut self, key: String, val: String);
}

/// A simple, persistable identity record: string→string map.
///
/// Used to hold identity-local metadata (e.g., the key manager contract address).
#[derive(Debug, Serialize, Deserialize, WeilType)]
pub struct IdentityToken {
    /// Backing storage for identity attributes.
    m: WeilMap<String, String>,
}

impl IdentityToken {
    /// Initialize a new identity and seed its **Key Manager** with management keys.
    ///
    /// This performs two actions:
    /// 1. Calls the key manager’s `add_keys` to grant `KeyPurpose::Management` for each
    ///    provided key (with `key_type = 1` by convention).
    /// 2. Creates the token storage and persists `"key_management_addr" = key_mgmt_addr`.
    ///
    /// # Arguments
    /// * `key_mgmt_addr` — Address/ID of the deployed Key Manager contract.
    /// * `keys` — Keys to be granted `Management` purpose at initialization.
    /// * `id_gen` — ID generator used to allocate the backing [`WeilMap`].
    pub fn init(key_mgmt_addr: String, keys: Vec<String>, id_gen: &mut WeilIdGenerator) -> Self {
        #[derive(Serialize)]
        struct Args {
            keys: Vec<(String, KeyPurpose, KeyType)>,
        }

        // Seed management keys on the external key manager. Errors are ignored here on purpose:
        // init continues so the identity storage is still created even if key seeding fails.
        let _ = Runtime::call_contract::<()>(
            key_mgmt_addr.clone(),
            "add_keys".to_string(),
            Some(
                serde_json::to_string(&Args {
                    keys: keys
                        .into_iter()
                        .map(|k| (k, KeyPurpose::Management, 1))
                        .collect(),
                })
                .unwrap(),
            ),
        );

        let mut token = IdentityToken {
            m: WeilMap::new(id_gen.next_id()),
        };

        token.set_data("key_management_addr".to_string(), key_mgmt_addr);

        token
    }

    /// Add a key with the specified `purpose` and `key_ty` to the configured Key Manager.
    ///
    /// This call is **permissioned**: the current **sender** (from [`Runtime::sender`]) must
    /// already possess `KeyPurpose::Management` on the Key Manager; otherwise an error is
    /// returned.
    ///
    /// # Errors
    /// - If the key manager address is missing in this token (no `"key_management_addr"`).
    /// - If the sender lacks the required management purpose.
    /// - If the underlying `add_key` contract call fails.
    pub fn add_key(
        &self,
        key: String,
        purpose: KeyPurpose,
        key_ty: KeyType,
    ) -> Result<(), anyhow::Error> {
        let addr = Runtime::sender();

        let Some(key_manager_addr) = self.get_data(&"key_management_addr".to_string()) else {
            return Err(anyhow::Error::msg("key manager address is not added"));
        };

        #[derive(Serialize)]
        struct KeyHasPurposeArgs {
            key: String,
            purpose: KeyPurpose,
        }

        // Authorization check: caller must have Management purpose.
        let args = serde_json::to_string(&KeyHasPurposeArgs {
            key: addr,
            purpose: KeyPurpose::Management,
        })
        .unwrap();

        let has_management_purpose = Runtime::call_contract::<bool>(
            key_manager_addr.clone(),
            "key_has_purpose".to_string(),
            Some(args),
        )?;

        if !has_management_purpose {
            return Err(anyhow::Error::msg(
                "sender address is not authorized to add key",
            ));
        }

        #[derive(Serialize)]
        struct AddKeyArgs {
            key: String,
            purpose: KeyPurpose,
            key_ty: KeyType,
        }

        let args = serde_json::to_string(&AddKeyArgs {
            key,
            purpose,
            key_ty,
        })
        .unwrap();

        let _ = Runtime::call_contract::<()>(key_manager_addr, "add_key".to_string(), Some(args))?;

        Ok(())
    }
}

impl Identity for IdentityToken {
    /// Fetch a stored value by key.
    fn get_data(&self, key: &String) -> Option<String> {
        self.m.get(key)
    }

    /// Store or overwrite a value by key.
    fn set_data(&mut self, key: String, val: String) {
        self.m.insert(key, val);
    }
}
