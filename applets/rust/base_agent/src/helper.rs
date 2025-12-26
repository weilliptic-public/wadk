
use serde::{Deserialize, Serialize};
use anyhow::Result;
use weil_rs::runtime::Runtime;


#[derive(Debug, Serialize, Deserialize)]
pub enum Model {
    QWEN_235B,
    GPT_5POINT1,
    CLAUDE_SONNET,
}


pub struct BaseAgentHelper {
    contract_id: String,
}

impl BaseAgentHelper {
    /// Creates a new BaseAgentHelper instance
    /// 
    /// # Arguments
    /// * `contract_id` - The contract ID of the base agent helper service
    pub fn new(contract_id: String) -> Self {
        BaseAgentHelper {
            contract_id,
        }
    }
}

impl BaseAgentHelper {
    /// Executes a task by calling the base agent helper contract
    /// 
    /// # Arguments
    /// * `task_prompt` - The prompt describing the task to execute
    /// * `mcp_contract_address` - The MCP contract address for model access
    /// * `model` - The AI model to use for task execution
    /// 
    /// # Returns
    /// The task execution result as a string
    pub fn run_task(&self, task_prompt: String, mcp_contract_address: String, model: Model) -> Result<String> {

        #[derive(Debug, Serialize)]
        struct run_taskArgs {
            task_prompt: String,
            mcp_contract_address: String,
            model: Model,
        }

        let serialized_args = Some(serde_json::to_string(&run_taskArgs { task_prompt, mcp_contract_address, model }).unwrap());

        let resp = Runtime::call_contract::<String>(
            self.contract_id.to_string(),
            "run_task".to_string(),
            serialized_args,
        )?;

        Ok(resp)
    }

}
