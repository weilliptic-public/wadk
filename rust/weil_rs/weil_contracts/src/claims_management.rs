/*use super::key_management::WRC734;
use crate::key_management::{KeyManager, KeyPurpose, KeyType, PublicKey};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use weil_macros::WeilType;
use weil_rs::{
    collections::{map::WeilMap, WeilIdGenerator},
    runtime::Runtime,
};

pub type ClaimId = String;
pub type Topic = u32;
pub type Scheme = u32;
pub type Address = String;
pub type Signature = String;
pub type Data = String;
pub type Uri = String;

pub trait ERC735 {
    fn get_claim(
        &self,
        claim_id: ClaimId,
    ) -> Result<(Topic, Scheme, Address, Signature, Data, Uri), anyhow::Error>;
    fn get_claimids_by_topic(&self, topic: Topic) -> Result<Vec<ClaimId>, anyhow::Error>;
    fn add_claim(
        &mut self,
        topic: Topic,
        scheme: Scheme,
        issuer: Address,
        signature: Signature,
        data: Data,
        uri: Uri,
    ) -> Result<ClaimId, anyhow::Error>;
    fn remove_claim(&mut self, claim_id: ClaimId) -> Result<bool, anyhow::Error>;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct Claim {
    topic: Topic,
    scheme: Scheme,
    issuer: Address,
    signature: Signature,
    data: Data,
    uri: Uri,
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct ClaimHolder {
    owner: Address,
    claims: WeilMap<ClaimId, Claim>,
    claimids_by_topic: WeilMap<Topic, BTreeSet<ClaimId>>,
    key_manager: KeyManager,
}

impl ClaimHolder {
    pub fn new(owner: Address, id_gen: &mut WeilIdGenerator) -> Self {
        Self {
            owner: owner.clone(),
            claims: WeilMap::new(id_gen.next_id()),
            claimids_by_topic: WeilMap::new(id_gen.next_id()),
            key_manager: KeyManager::new(owner, id_gen),
        }
    }

    pub fn get_key(&self, key: PublicKey) -> (Vec<KeyPurpose>, KeyType, PublicKey) {
        self.key_manager.get_key(key)
    }

    pub fn key_has_purpose(&self, key: PublicKey, purpose: KeyPurpose) -> bool {
        self.key_manager.key_has_purpose(key, purpose)
    }

    pub fn get_keys_by_purpose(&self, purpose: KeyPurpose) -> Vec<PublicKey> {
        self.key_manager.get_keys_by_purpose(purpose)
    }

    pub fn add_key(
        &mut self,
        key: PublicKey,
        purpose: KeyPurpose,
        key_type: KeyType,
    ) -> Result<bool, anyhow::Error> {
        self.key_manager.add_key(key, purpose, key_type)
    }

    pub fn remove_key(
        &mut self,
        key: PublicKey,
        purpose: KeyPurpose,
    ) -> Result<bool, anyhow::Error> {
        self.key_manager.remove_key(key, purpose)
    }
}

impl ERC735 for ClaimHolder {
    fn get_claim(
        &self,
        claim_id: ClaimId,
    ) -> Result<(Topic, Scheme, Address, Signature, Data, Uri), anyhow::Error> {
        if let Some(claim) = self.claims.get(&claim_id) {
            return Ok((
                claim.topic,
                claim.scheme,
                claim.issuer,
                claim.signature,
                claim.data,
                claim.uri,
            ));
        }

        Err(WasmHostInterfaceError::new_key_not_found_in_collection_error("Claim not found".into()))
    }

    fn get_claimids_by_topic(&self, topic: Topic) -> Result<Vec<ClaimId>, WasmHostInterfaceError> {
        if let Some(claim_ids) = self.claimids_by_topic.get(&topic) {
            return Ok(claim_ids.into_iter().collect());
        }

        Ok(Vec::new())
    }

    fn add_claim(
        &mut self,
        topic: Topic,
        scheme: Scheme,
        issuer: Address,
        signature: Signature,
        data: Data,
        uri: Uri,
    ) -> Result<ClaimId, anyhow::Error> {
        if !self.owner.eq(&Runtime::get_sender()) {
            return Err(WasmHostInterfaceError::new_function_returned_with_error(
                "add_claim".into(),
                "Sender does not own the claim",
            ));
        }

        let mut sha256 = Sha256::new();
        sha256.update(issuer.as_bytes());
        sha256.update(&topic.to_be_bytes());
        let claim_id_bytes: [u8; 32] = sha256.finalize().into();
        let claim_id: ClaimId = hex::encode(claim_id_bytes);

        if let Some(claim) = self.claims.get(&claim_id) {
            if !claim.issuer.eq(&issuer) || claim.topic != topic {
                return Err(WasmHostInterfaceError::new_function_returned_with_error(
                    "add_claim".into(),
                    "ClaimId conflict with existing one",
                ));
            }
        } else {
            let claim = Claim {
                topic,
                scheme,
                issuer,
                signature,
                data,
                uri,
            };
            self.claims.insert(claim_id.clone(), claim);

            let mut claim_ids = match self.claimids_by_topic.get(&topic) {
                Some(val) => val,
                None => BTreeSet::new(),
            };
            claim_ids.insert(claim_id.clone());
            self.claimids_by_topic.insert(topic, claim_ids);
        }

        Ok(claim_id)
    }

    fn remove_claim(&mut self, claim_id: ClaimId) -> Result<bool, anyhow::Error> {
        if !self.owner.eq(&Runtime::sender()) {
            return Err(WasmHostInterfaceError::new_function_returned_with_error(
                "remove_claim".into(),
                "Sender does not own the claim",
            ));
        }

        if let Some(claim) = self.claims.remove(&claim_id) {
            if let Some(mut claim_ids) = self.claimids_by_topic.get(&claim.topic) {
                if claim_ids.remove(&claim_id) {
                    self.claimids_by_topic.insert(claim.topic, claim_ids);
                }
            }
        }

        Ok(true)
    }
}*/
