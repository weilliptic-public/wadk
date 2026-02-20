//! Flow registry for storing and retrieving execution context by namespace and flow ID.

use crate::runtime::Runtime;
use anyhow::Result;
use serde::Serialize;

/// Registry client for flow execution context (get/persist) via contract calls.
pub struct FlowRegistry {
    contract_id: String,
}

impl FlowRegistry {
    /// Creates a new [`FlowRegistry`] bound to the given flow registry contract.
    ///
    /// # Arguments
    ///
    /// * `contract_id` - The contract ID of the flow registry to use.
    pub fn new(contract_id: String) -> Self {
        FlowRegistry { contract_id }
    }
}

impl FlowRegistry {
    /// Fetches the persisted execution context for a flow, if any.
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace under which the flow is registered.
    /// * `flow_id` - Unique identifier of the flow within the namespace.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(ctx))` - Persisted context string when present.
    /// * `Ok(None)` - No context stored for this namespace/flow_id.
    ///
    /// # Errors
    ///
    /// Returns an error if the contract call fails.
    pub fn get_execution_context(
        &self,
        namespace: String,
        flow_id: String,
    ) -> Result<Option<String>> {
        #[derive(Debug, Serialize)]
        struct get_execution_contextArgs {
            namespace: String,
            flow_id: String,
        }

        let serialized_args =
            Some(serde_json::to_string(&get_execution_contextArgs { namespace, flow_id }).unwrap());

        let resp = Runtime::call_contract::<Option<String>>(
            self.contract_id.to_string(),
            "get_execution_context".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

    /// Persists execution context for a flow under the given namespace and flow ID.
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace under which to store the flow.
    /// * `flow_id` - Unique identifier of the flow within the namespace.
    /// * `ctx` - Serialized execution context string to persist.
    ///
    /// # Errors
    ///
    /// Returns an error if the contract call fails.
    pub fn persist_execution_context(
        &self,
        namespace: String,
        flow_id: String,
        ctx: String,
    ) -> Result<()> {
        #[derive(Debug, Serialize)]
        struct persist_execution_contextArgs {
            namespace: String,
            flow_id: String,
            ctx: String,
        }

        let serialized_args = Some(
            serde_json::to_string(&persist_execution_contextArgs {
                namespace,
                flow_id,
                ctx,
            })
            .unwrap(),
        );

        let resp = Runtime::call_contract::<()>(
            self.contract_id.to_string(),
            "persist_execution_context".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }
}
