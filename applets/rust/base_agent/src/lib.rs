use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, mutate, query, secured, smart_contract};
use weil_rs::runtime::Runtime;

mod helper;
use helper::{BaseAgentHelper, Model};

const BASE_AGENT_HELPER_NAME: &str = "base_agent_helper::weil";

trait BaseAgent {
    fn new(description: String, mcp_contract_address: String) -> Result<Self, String>
    where
        Self: Sized;
    async fn run_task(&self, task_prompt: String) -> Result<String, String>;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct BaseAgentContractState {
    // define your contract state here!
    description: String,
    mcp_contract_address: String,
}

#[smart_contract]
impl BaseAgent for BaseAgentContractState {
    /// Creates a new BaseAgent contract instance
    /// 
    /// # Arguments
    /// * `description` - Description of the base agent's purpose
    /// * `mcp_contract_address` - Contract address for the MCP (Model Context Protocol) service
    #[constructor]
    fn new(description: String, mcp_contract_address: String) -> Result<Self, String>
    where
        Self: Sized,
    {
        let base_agent = BaseAgentContractState {
            description,
            mcp_contract_address,
        };

        Ok(base_agent)
    }

    /// Executes a task based on the provided prompt using the configured AI model
    /// 
    /// # Arguments
    /// * `task_prompt` - The prompt describing the task to be executed
    /// 
    /// # Returns
    /// The result of the task execution as a string
    #[query]
    async fn run_task(&self, task_prompt: String) -> Result<String, String> {
        // safe to unwrap.
        let base_agent_helper_address =
            Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
        let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

        let res = base_agent_helper
            .run_task(
                task_prompt,
                self.mcp_contract_address.clone(),
                Model::GPT_5POINT1,
            )
            .map_err(|e| e.to_string())?;

        Ok(res)
    }
}
