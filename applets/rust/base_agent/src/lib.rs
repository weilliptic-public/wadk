use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::runtime::Runtime;
use weil_rs::ai::agents::{base::BaseAgentHelper, Model};

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

        // Argument order follows `BaseAgentHelper::run_task`: the MCP
        // addresses and their server names come first, the prompt last.
        // `mcp_contract_addresses` is a Vec, so the single address held in
        // state is wrapped rather than passed bare.
        let res = base_agent_helper
            .run_task(
                vec![self.mcp_contract_address.clone()],
                // No server names: this applet's state carries only an MCP
                // address. If the helper requires a name per address, add a
                // `server_names` field to the state and pass it here.
                vec![],
                Model::GPT_5POINT1,
                Some("<api_key>".to_string()),
                task_prompt,
            )
            .map_err(|e| e.to_string())?;

        Ok(res)
    }
}
