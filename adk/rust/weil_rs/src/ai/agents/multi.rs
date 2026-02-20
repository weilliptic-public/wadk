//! Multi-agent helper for running multiple tasks via a task executor contract.

use super::errors::TaskExecutorError;
use super::models::Model;
use crate::runtime::Runtime;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Result of a multi-task execution from the contract.
#[derive(Debug, Serialize, Deserialize)]
pub enum TaskExecutorResult {
    /// All tasks completed successfully; contains the final result string.
    Ok(String),
    /// A task failed; contains (error_message, resume_task_index, previous_task_result).
    Err((String, u32, Option<String>)),
}

/// Helper for interacting with a multi-agent (task executor) contract.
pub struct MultiAgentHelper {
    contract_id: String,
}

impl MultiAgentHelper {
    /// Creates a new [`MultiAgentHelper`] bound to the given task executor contract.
    ///
    /// # Arguments
    ///
    /// * `contract_id` - The contract ID of the multi-agent / task executor to invoke.
    pub fn new(contract_id: String) -> Self {
        MultiAgentHelper { contract_id }
    }
}

impl MultiAgentHelper {
    /// Runs multiple tasks in sequence by calling the contract's `run_tasks` entrypoint.
    ///
    /// If a task fails, the error includes the index at which to resume and the remaining
    /// task descriptions.
    ///
    /// # Arguments
    ///
    /// * `task_prompts` - Slice of natural language prompts, one per task.
    /// * `mcp_contract_addresses` - Addresses of MCP contracts to use for tooling.
    /// * `model` - The model to use for task execution.
    ///
    /// # Returns
    ///
    /// The final task result as a string when all tasks succeed.
    ///
    /// # Errors
    ///
    /// Returns a [`TaskExecutorError`] containing resume index and remaining tasks if a task
    /// fails; or an error if the contract call itself fails.
    pub fn run_tasks<'a>(
        &'a self,
        task_prompts: &'a [String],
        mcp_contract_addresses: Vec<String>,
        model: Model,
    ) -> Result<String, anyhow::Error> {
        #[derive(Debug, Serialize)]
        struct RunTaskArgs<'a> {
            task_prompts: &'a [String],
            mcp_contract_addresses: Vec<String>,
            model: Model,
        }

        let serialized_args = Some(
            serde_json::to_string(&RunTaskArgs {
                task_prompts,
                mcp_contract_addresses,
                model,
            })
            .unwrap(),
        );

        let resp = Runtime::call_contract::<TaskExecutorResult>(
            self.contract_id.to_string(),
            "run_tasks".to_string(),
            serialized_args,
        )?;

        let resp = {
            match resp {
                TaskExecutorResult::Ok(resp) => Ok(resp),
                TaskExecutorResult::Err((_, resume_task_index, previous_task_result)) => {
                    #[derive(Serialize)]
                    struct TaskError<'a> {
                        pub resume_task_index: u32,
                        pub previous_task_result: Option<String>,
                        pub remaining_task_descriptions: &'a [String],
                    }

                    Err(TaskExecutorError {
                        serialized_err: serde_json::to_string(&TaskError {
                            resume_task_index,
                            previous_task_result,
                            remaining_task_descriptions: &task_prompts
                                [resume_task_index as usize..],
                        })
                        .unwrap(),
                    })
                }
            }
        }?;

        Ok(resp)
    }
}
