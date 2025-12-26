
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use weil_macros::{constructor, query, smart_contract, WeilType};
use weil_rs::runtime::Runtime;

mod helper;
mod utils;
mod registry;
use helper::{BaseAgentHelper, Model};
use registry::FlowRegistry;

const BASE_AGENT_HELPER_NAME: &str = "base_agent_helper::weil";
const FLOW_REGISTRY_NAME: &str = "flow_registry::weil";


#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, WeilType, Hash)]
pub enum Step {
    A,
    B,
    C,
    D,
    E
}

pub type Prompt = String;

#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct PromptPlan {
    prompts: BTreeMap<Step, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RunStatus {
    Continue(Option<Step>),
    Failed,
    Pending,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct ExecutionContext {
    step: Option<Step>,
    answers: Vec<String>,
    prompt_plan: PromptPlan,
    model: Model,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            step: Some(Step::A),
            answers: vec![],
            prompt_plan: PromptPlan {
                prompts: BTreeMap::new(),
            },
            model: Model::QWEN_235B,
        }
    }
}

pub(crate) trait State: Send + Sync {
    /// Takes ownership of ctx, returns updated ctx + status.
    fn do_work(&self, mcp_addresses: Vec<String>, ctx: ExecutionContext) -> (ExecutionContext, RunStatus);
    fn next_step(&self) -> Option<Step>;
    fn name(&self) -> Step;
}

trait StepAgent {
    fn new(mcp_contract_addresses: Vec<String>) -> Result<Self, String>
    where
        Self: Sized;
    async fn run(&self, namespace: String, flow_id: String, execution_context: ExecutionContext) -> (RunStatus, ExecutionContext);
    async fn resume(&self, namespace: String, flow_id: String) -> (RunStatus, ExecutionContext);
}


#[derive(Serialize, Deserialize)]
pub struct StepAgentContractState {
    // define your contract state here!
    mcp_contract_addresses: Vec<String>,
    #[serde(with = "utils::handlers_serde")]
    handlers: BTreeMap<Step, Box<dyn State>>,
}

impl weil_rs::traits::WeilType for StepAgentContractState {}

#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub(crate) struct AgentState{
    mcp_address: String,
    prompt: String,
}

#[smart_contract]
impl StepAgent for StepAgentContractState {
    /// Creates a new StepAgent contract instance with registered state handlers
    /// 
    /// # Arguments
    /// * `mcp_contract_addresses` - List of MCP contract addresses for AI model access
    #[constructor]
    fn new(mcp_contract_addresses: Vec<String>) -> Result<Self, String>
    where
        Self: Sized,
    {        
        let mut handlers: BTreeMap<Step, Box<dyn State>> = BTreeMap::new();
        
        handlers.insert(Step::A, Box::new(StateA));
        handlers.insert(Step::B, Box::new(StateB));
        handlers.insert(Step::C, Box::new(StateC));
        handlers.insert(Step::D, Box::new(StateD));
        handlers.insert(Step::E, Box::new(StateE));

        Ok(Self {
            mcp_contract_addresses,
            handlers,
        })
    }


    /// Executes a multi-step workflow starting from the current execution context
    /// 
    /// # Arguments
    /// * `namespace` - The namespace for organizing workflows
    /// * `flow_id` - Unique identifier for this workflow execution
    /// * `execution_context` - The current execution context containing state and progress
    /// 
    /// # Returns
    /// A tuple of (RunStatus, ExecutionContext) indicating the workflow status and updated context
    #[query]
    async fn run(&self, namespace: String, flow_id: String, execution_context: ExecutionContext) -> (RunStatus, ExecutionContext) {

        let mut context = execution_context;
        
        loop {
            let Some(step) = context.step.clone() else {
                // No step => already complete
                return (RunStatus::Done, context);
            };

            let Some(handler) = self.handlers.get(&step) else {
                // Unknown step: treat as failure (or done; choose what fits your semantics)
                return (RunStatus::Failed, context);
            };

            // Apply the step handler (THIS is the authoritative state transition)
            let (new_ctx, status) = handler.do_work(
                self.mcp_contract_addresses.clone(),
                context,
            );
            context = new_ctx;

            match status {
                RunStatus::Continue(next) => {
                    // If handler returned an explicit next step, set it.
                    // Otherwise, respect whatever ctx.step already contains.
                    if let Some(next_step) = next {
                        context.step = Some(next_step);
                    }
                    // If neither set a next step, we are done.
                    if context.step.is_none() {
                        return (RunStatus::Done, context);
                    }
                    continue;
                }
                RunStatus::Pending => {
                    // IMPORTANT: ctx.step should already point at where to resume
                    // (either same step or next step, your choice).
                    // Respect what the handler set in ctx.step, don't override it
                    return (RunStatus::Pending, context);
                }
                RunStatus::Done => return (RunStatus::Done, context),
                RunStatus::Failed => return (RunStatus::Failed, context),
            }
        }
    }

    /// Resumes a previously paused workflow by loading its execution context from the registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the workflow is stored
    /// * `flow_id` - The unique identifier of the workflow to resume
    /// 
    /// # Returns
    /// A tuple of (RunStatus, ExecutionContext) indicating the workflow status and current context
    #[query]
    async fn resume(&self, namespace: String, flow_id: String) -> (RunStatus, ExecutionContext) {
        let flow_registry_address = Runtime::contract_id_for_name(FLOW_REGISTRY_NAME).unwrap();
        let flow_registry = FlowRegistry::new(flow_registry_address);

        let serialized_ctx = flow_registry.get_execution_context(namespace.clone(), flow_id.clone()).unwrap();
        // we MUST get a context to proceed
        let ctx = serialized_ctx.unwrap();

        let context = serde_json::from_str::<ExecutionContext>(&ctx).unwrap();

        self.run(namespace, flow_id, context).await
    }

}


pub(crate) struct StateA;
impl State for StateA {

    /// Returns the step identifier for this state
    fn name(&self) -> Step {
        Step::A
    }

    /// Returns the next step to transition to after this state completes
    fn next_step(&self) -> Option<Step> {
        Some(Step::B)
    }

    /// Executes the work for Step A, processing any prompts and transitioning state
    /// 
    /// # Arguments
    /// * `mcp_addresses` - List of MCP contract addresses for AI model access
    /// * `ctx` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (ExecutionContext, RunStatus) with updated context and execution status
    fn do_work(&self, mcp_addresses: Vec<String>, mut ctx: ExecutionContext) -> (ExecutionContext, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(&self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (ctx, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

       println!("ran step A");

        // Decide next step explicitly via Continue(Some(...))
        (ctx, RunStatus::Continue(Some(Step::B)))
    }
}

pub(crate) struct StateB;
impl State for StateB {
    /// Returns the step identifier for this state
    fn name(&self) -> Step {
        Step::B
    }

    /// Returns the next step to transition to after this state completes
    fn next_step(&self) -> Option<Step> {
        Some(Step::C)
    }

    /// Executes the work for Step B, processing any prompts and transitioning state
    /// 
    /// # Arguments
    /// * `mcp_addresses` - List of MCP contract addresses for AI model access
    /// * `ctx` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (ExecutionContext, RunStatus) with updated context and execution status
    fn do_work(&self, mcp_addresses: Vec<String>, mut ctx: ExecutionContext) -> (ExecutionContext, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(&self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (ctx, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step B");
        // Move to next step after completing work
        ctx.step = Some(Step::C);

        (ctx, RunStatus::Pending)
    }
}


pub(crate) struct StateC;
impl State for StateC {

    /// Returns the step identifier for this state
    fn name(&self) -> Step {
        Step::C
    }
    /// Returns the next step to transition to after this state completes
    fn next_step(&self) -> Option<Step> {
         Some(Step::D)
    }

    /// Executes the work for Step C, processing any prompts and transitioning state
    /// 
    /// # Arguments
    /// * `mcp_addresses` - List of MCP contract addresses for AI model access
    /// * `ctx` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (ExecutionContext, RunStatus) with updated context and execution status
    fn do_work(&self, mcp_addresses: Vec<String>, mut ctx: ExecutionContext) -> (ExecutionContext, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(&self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (ctx, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step C");
        ctx.step = Some(Step::D);

        (ctx, RunStatus::Pending)
    }
}


pub(crate) struct StateD;
impl State for StateD {

    /// Returns the step identifier for this state
    fn name(&self) -> Step {
        Step::D
    }
    /// Returns the next step to transition to after this state completes
    fn next_step(&self) -> Option<Step> {
       Some(Step::E)
    }

    /// Executes the work for Step D, processing any prompts and transitioning state
    /// 
    /// # Arguments
    /// * `mcp_addresses` - List of MCP contract addresses for AI model access
    /// * `ctx` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (ExecutionContext, RunStatus) with updated context and execution status
    fn do_work(&self, mcp_addresses: Vec<String>, mut ctx: ExecutionContext) -> (ExecutionContext, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(&self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (ctx, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step D");
        ctx.step = Some(Step::E);

        (ctx, RunStatus::Pending)
    }
}

pub(crate) struct StateE;
impl State for StateE {

    /// Returns the step identifier for this state
    fn name(&self) -> Step {
        Step::E
    }
    /// Returns None as this is the terminal step
    fn next_step(&self) -> Option<Step> {
        None
    }

    /// Executes the work for Step E (final step), processing any prompts and marking completion
    /// 
    /// # Arguments
    /// * `mcp_addresses` - List of MCP contract addresses for AI model access
    /// * `ctx` - The current execution context
    /// 
    /// # Returns
    /// A tuple of (ExecutionContext, RunStatus) with updated context and Done status
    fn do_work(&self, mcp_addresses: Vec<String>, mut ctx: ExecutionContext) -> (ExecutionContext, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(&self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (ctx, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step E");
        ctx.step = None; // terminal

        (ctx, RunStatus::Done)
    }
}