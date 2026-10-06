//! Base agent helper for running single tasks via contract calls.

use crate::{ai::agents::Model, runtime::Runtime};
use anyhow::Result;
use serde::Serialize;

/// Helper for interacting with a base (single-task) agent contract.
pub struct BaseAgentHelper {
    contract_id: String,
}

impl BaseAgentHelper {
    /// Creates a new [`BaseAgentHelper`] bound to the given agent contract.
    ///
    /// # Arguments
    ///
    /// * `contract_id` - The contract ID of the base agent to invoke.
    pub fn new(contract_id: String) -> Self {
        BaseAgentHelper { contract_id }
    }
}

impl BaseAgentHelper {
    /// Runs a single task by calling the agent contract's `run_task` entrypoint.
    ///
    /// # Arguments
    ///
    /// * `task_prompt` - The natural language prompt describing the task.
    ///
    /// # Returns
    ///
    /// The task result as a string on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the contract call fails or the response cannot be parsed.
    pub fn run_task(&self, mcp_contract_addresses: Vec<String>, server_names: Vec<String>, model: Model, model_key: Option<String>, task_prompt: String) -> Result<String> {

        #[derive(Debug, Serialize)]
        struct run_taskArgs {
            mcp_contract_addresses: Vec<String>,
            server_names: Vec<String>,
            model: Model,
            model_key: Option<String>,
            task_prompt: String,
        }

        let serialized_args = Some(serde_json::to_string(&run_taskArgs { mcp_contract_addresses, server_names, model, model_key, task_prompt }).unwrap());

        let resp = Runtime::call_contract::<String>(
            self.contract_id.to_string(),
            "run_task".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }
}
