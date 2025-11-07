use crate::errors::InvalidContractIdError;
use base32::{decode, Alphabet};
use serde::{de::Visitor, Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};

/// Represents the contract id (sometimes also called "contract address") of the Weil Applet (smart contract).
#[derive(Debug, Clone)]
pub struct ContractId(Arc<String>);

impl ContractId {
    /// Constructs a new contract id from a string.
    pub fn new(contract_id: String) -> Result<Self, InvalidContractIdError> {
        ContractId::validate_contract_id_str(&contract_id)?;

        Ok(ContractId(Arc::new(contract_id)))
    }

    pub(crate) fn pod_counter(&self) -> Result<i32, anyhow::Error> {
        let Some(decoded_bytes) = decode(Alphabet::Rfc4648Lower { padding: false }, &self.0) else {
            return Err(anyhow::Error::msg(
                "unable to decode the applet id".to_string(),
            ));
        };

        // Extract the first 4 bytes as the u32 pod_id (WeilPodIdCounter)
        let pod_id_bytes: [u8; 4] = decoded_bytes[..4].try_into()?;

        let pod_id_counter = i32::from_be_bytes(pod_id_bytes);

        Ok(pod_id_counter)
    }

    fn validate_contract_id_str(s: &str) -> Result<(), InvalidContractIdError> {
        // TODO - check the validatiy for contract id
        Ok(())
    }
}

impl Serialize for ContractId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_ref())
    }
}

struct ContractIdVisitor;

impl<'de> Visitor<'de> for ContractIdVisitor {
    type Value = ContractId;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct ContractId")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ContractId(Arc::new(v.to_string())))
    }
}

impl<'de> Deserialize<'de> for ContractId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(ContractIdVisitor)
    }
}

impl ToString for ContractId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl FromStr for ContractId {
    type Err = InvalidContractIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ContractId::validate_contract_id_str(s)?;

        Ok(ContractId(Arc::new(s.to_string())))
    }
}
