
use serde::{Deserialize, Serialize};
use anyhow::Result;
use weil_rs::runtime::Runtime;


pub struct FlowRegistry {
    contract_id: String,
}

impl FlowRegistry {
    /// Creates a new FlowRegistry instance
    /// 
    /// # Arguments
    /// * `contract_id` - The contract ID of the flow registry service
    pub fn new(contract_id: String) -> Self {
        FlowRegistry {
            contract_id,
        }
    }
}

impl FlowRegistry {
    /// Retrieves a stored execution context from the registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the context is stored
    /// * `flow_id` - The unique identifier of the flow
    /// 
    /// # Returns
    /// An optional JSON string representing the execution context
    pub fn get_execution_context(&self, namespace: String, flow_id: String) -> Result<Option<String>> {

        #[derive(Debug, Serialize)]
        struct get_execution_contextArgs {
            namespace: String,
            flow_id: String,
        }

        let serialized_args = Some(serde_json::to_string(&get_execution_contextArgs { namespace, flow_id }).unwrap());

        let resp = Runtime::call_contract::<Option<String>>(
            self.contract_id.to_string(),
            "get_execution_context".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

    /// Stores an execution context in the registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the context should be stored
    /// * `flow_id` - The unique identifier of the flow
    /// * `ctx` - The JSON string representation of the execution context
    pub fn persist_execution_context(&self, namespace: String, flow_id: String, ctx: String) -> Result<()> {

        #[derive(Debug, Serialize)]
        struct persist_execution_contextArgs {
            namespace: String,
            flow_id: String,
            ctx: String,
        }

        let serialized_args = Some(serde_json::to_string(&persist_execution_contextArgs { namespace, flow_id, ctx }).unwrap());

        let resp = Runtime::call_contract::<()>(
            self.contract_id.to_string(),
            "persist_execution_context".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

}
