use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, mutate, query, smart_contract};
use weil_rs::runtime::Runtime;
use weil_rs::ai::agents::{multi::MultiAgentHelper, Model};


const MULTI_AGENT_HELPER_NAME: &str = "multi_agent_helper::weil";

trait MultiAgent {
    fn new(description: String, agent_addresses: Vec<String>) -> Result<Self, String>
    where
        Self: Sized;
    async fn run_tasks(&self, task_descriptions: Vec<String>) -> Result<String, String>;
    async fn update_resume_task_index(&mut self, index: u32, previous_task_result: Option<String>);
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct MultiAgentContractState {
    // define your contract state here!
    description: String,
    agent_addresses: Vec<String>,
    resume_task_index: u32,
    optional_task_previous_result: Option<String>,
}

#[smart_contract]
impl MultiAgent for MultiAgentContractState {
    /// Creates a new MultiAgent contract instance
    /// 
    /// # Arguments
    /// * `description` - Description of the multi-agent system's purpose
    /// * `agent_addresses` - List of contract addresses for individual agents
    #[constructor]
    fn new(description: String, agent_addresses: Vec<String>) -> Result<Self, String>
    where
        Self: Sized,
    {
        let multi_agent = MultiAgentContractState {
            description,
            agent_addresses,
            resume_task_index: 0,
            optional_task_previous_result: None,
        };

        Ok(multi_agent)
    }

    /// Executes multiple tasks across different agents
    /// 
    /// # Arguments
    /// * `task_descriptions` - List of task descriptions to be executed
    /// 
    /// # Returns
    /// The aggregated result of all task executions
    #[query]
    async fn run_tasks(&self, task_descriptions: Vec<String>) -> Result<String, String> {
        // safe to unwrap here.
        let multi_agent_helper_address =
            Runtime::contract_id_for_name(MULTI_AGENT_HELPER_NAME).unwrap();
        let multi_agent = MultiAgentHelper::new(multi_agent_helper_address);
        let resume_task_index = self.resume_task_index as usize;

        let resp = multi_agent
            .run_tasks(
                &task_descriptions[resume_task_index..],
                self.agent_addresses.clone(),
                Model::GPT_5POINT1,
                Some("<api_key>".to_string())
            )
            .map_err(|err| err.to_string())?;

        Ok(resp)
    }

    /// Updates the resume task index to allow continuing from a specific task
    /// 
    /// # Arguments
    /// * `index` - The task index from which to resume execution
    /// * `previous_task_result` - Optional result from the previous task execution
    #[mutate]
    async fn update_resume_task_index(&mut self, index: u32, previous_task_result: Option<String>) {
        self.resume_task_index = index;
        self.optional_task_previous_result = previous_task_result;
    }
}
