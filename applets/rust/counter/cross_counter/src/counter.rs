
use serde::{Deserialize, Serialize};
use anyhow::Result;
use weil_rs::runtime::Runtime;


pub struct Counter {
    contract_id: String,
}

impl Counter {
    pub fn new(contract_id: String) -> Self {
        Counter {
            contract_id,
        }
    }
}

impl Counter {
    pub fn get_count(&self) -> Result<usize> {
        let serialized_args = None;

        let resp = Runtime::call_contract::<usize>(
            self.contract_id.clone(),
            "get_count".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

    pub fn increment(&self) -> Result<()> {
        let serialized_args = None;

        let resp = Runtime::call_contract::<()>(
            self.contract_id.clone(),
            "increment".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

    pub fn set_value(&self, val: usize) -> Result<()> {

        #[derive(Debug, Serialize)]
        struct set_valueArgs {
            val: usize,
        }

        let serialized_args = Some(serde_json::to_string(&set_valueArgs { val })?);

        let resp = Runtime::call_contract::<()>(
            self.contract_id.clone(),
            "set_value".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }
}
