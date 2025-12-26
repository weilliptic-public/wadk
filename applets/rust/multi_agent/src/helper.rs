use anyhow::Result;
use serde::{Deserialize, Serialize};
use weil_rs::runtime::Runtime;

use crate::error::TaskExecutorError;

#[derive(Debug, Serialize, Deserialize)]
pub enum Model {
    QWEN_235B,
    GPT_5POINT1,
    CLAUDE_SONNET,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskExecutorResult {
    Ok(String),
    Err((String, u32, Option<String>)),
}

pub struct MultiAgentHelper {
    contract_id: String,
}

impl MultiAgentHelper {
    pub fn new(contract_id: String) -> Self {
        MultiAgentHelper { contract_id }
    }
}

impl MultiAgentHelper {
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
