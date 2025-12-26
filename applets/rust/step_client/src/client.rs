use serde::{Deserialize, Serialize};
use wadk_utils::errors::WeilError;
use weil_macros::WeilType;
use weil_rs::runtime::Runtime;
use weil_wallet::{contract::ContractId, wallet::Wallet, WeilClient, WeilContractClient};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

use crate::registry::FlowRegistryClient;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, WeilType, Hash)]
pub enum Step {
    A,
    B,
    C,
    D,
    E
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RunStatus {
    Continue(Option<Step>),
    Failed,
    Pending,
    Done,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptPlan {
    pub prompts: BTreeMap<Step, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Model {
    QWEN_235B,
    GPT_5POINT1,
    CLAUDE_SONNET,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub step: Option<Step>,
    pub answers: Vec<String>,
    pub prompt_plan: PromptPlan,
    pub model: Model,
}

pub struct StepAgentClient {
    client: WeilContractClient,
    flow_registry_client: FlowRegistryClient
}

const FLOW_REGISTRY_NAME: &str = "flow_registry::weil";

impl StepAgentClient {
    /// Creates a new StepAgentClient instance
    /// 
    /// # Arguments
    /// * `contract_id` - The contract ID of the step agent
    /// * `wallet` - The wallet to use for signing transactions
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {

        // let flow_registry_address = Runtime::contract_id_for_name(FLOW_REGISTRY_NAME).unwrap();
        let flow_registry_address = "".to_string();
        let flow_registry_contract_id = flow_registry_address.parse::<ContractId>().unwrap();

        Ok(StepAgentClient {
            client: WeilClient::new(wallet.clone(), None)?.to_contract_client(contract_id.clone()),
            flow_registry_client: FlowRegistryClient::new(flow_registry_contract_id,wallet).unwrap()
        })
    }

    /// Initiates a new workflow execution with the provided execution context
    /// 
    /// # Arguments
    /// * `namespace` - The namespace for organizing workflows
    /// * `flow_id` - Unique identifier for this workflow execution
    /// * `execution_context` - The initial execution context for the workflow
    /// 
    /// # Returns
    /// A tuple of (RunStatus, ExecutionContext) indicating the workflow status and current context
    pub async fn run(&self, namespace: String, flow_id: String, execution_context: ExecutionContext) -> Result<(RunStatus, ExecutionContext), anyhow::Error> {

        if let Some(_) = self.flow_registry_client.get_execution_context(namespace.clone(), flow_id.clone()).await? {
            return Err(anyhow!("Context already exists. Workflow in progress."));
        }

        self.flow_registry_client.persist_execution_context(namespace.clone(), flow_id.clone(), serde_json::to_string(&execution_context).unwrap()).await?;
        self.run_helper(namespace.clone(), flow_id.clone(), execution_context).await
    }

    /// Resumes a previously paused workflow by loading its execution context
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the workflow is stored
    /// * `flow_id` - The unique identifier of the workflow to resume
    /// 
    /// # Returns
    /// A tuple of (RunStatus, ExecutionContext) indicating the workflow status and current context
    pub async fn resume(&self, namespace: String, flow_id: String) -> Result<(RunStatus, ExecutionContext), anyhow::Error> {
        
        let ctx = self.flow_registry_client.get_execution_context(namespace.clone(), flow_id.clone()).await?;

        if ctx.is_none() {
            return Err(anyhow!("No existing context found. Cannot resume."));
        }
        let execution_context = serde_json::from_str::<ExecutionContext>(&ctx.as_ref().unwrap()).unwrap();
        self.run_helper(namespace.clone(), flow_id.clone(), execution_context).await
    }

    /// Internal helper method to execute the workflow and handle context persistence
    /// 
    /// # Arguments
    /// * `namespace` - The namespace for the workflow
    /// * `flow_id` - The unique identifier of the workflow
    /// * `execution_context` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (RunStatus, ExecutionContext) with the execution result
    async fn run_helper(&self, namespace: String, flow_id: String, execution_context: ExecutionContext) -> Result<(RunStatus, ExecutionContext), anyhow::Error> {
     #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
            execution_context: ExecutionContext,
        }

        let args = Args { 
            namespace: namespace.clone(), 
            flow_id: flow_id.clone(), 
            execution_context 
        };

        let resp = self.client.execute("run".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<(RunStatus, ExecutionContext)>(&result)?;

        match result.0 {
            RunStatus::Pending => {
                self.flow_registry_client.persist_execution_context(namespace, flow_id, serde_json::to_string(&result.1).unwrap()).await?;
            },
            RunStatus::Done => {
                self.flow_registry_client.delete_context(namespace, flow_id).await?;
            }
            RunStatus::Failed => {
                self.flow_registry_client.delete_context(namespace, flow_id).await?;
            }
            _ => {}
        }

        Ok(result)
    }

}