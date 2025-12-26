
use serde::{Deserialize, Serialize};
use anyhow::Result;
use weil_macros::WeilType;
use weil_rs::runtime::Runtime;


#[derive(Debug, Serialize, Deserialize, Clone, WeilType)]
pub enum Model {
    QWEN_235B,
    GPT_5POINT1,
    CLAUDE_SONNET,
}


pub struct BaseAgentHelper {
    contract_id: String,
}

impl BaseAgentHelper {
    pub fn new(contract_id: String) -> Self {
        BaseAgentHelper {
            contract_id,
        }
    }
}

impl BaseAgentHelper {
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
