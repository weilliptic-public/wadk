//! Step agent smart contract: runs a state-machine flow with named steps (A–E) and resumable execution.

use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, mutate, query, smart_contract};
use weil_rs::{ai::agents::{Model, base::BaseAgentHelper, flow_registry::FlowRegistry}};
use std::collections::BTreeMap;
use weil_rs::runtime::Runtime;

const BASE_AGENT_HELPER_NAME: &str = "base_agent_helper::weil";
const FLOW_REGISTRY_NAME: &str = "flow_registry::weil";
const MAX_CONTEXT_SIZE: usize = 1024;

/// Handler for a single step in the flow; performs work and returns the next step and status.
#[typetag::serde(tag = "type")]
pub(crate) trait State: Send + Sync {
    /// Unique name of this step (e.g. `"A"`, `"B"`).
    fn name(&self) -> &'static str;
    /// Runs this step: may mutate `ctx` and returns the next step (if any) and run status.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus);
}

/// Named steps in the step-agent flow (A → B → C → D → E).
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Hash, WeilType)]
pub enum Step {
    A,
    B,
    C,
    D,
    E,
}

/// Outcome of running a step or the full flow.
#[derive(Debug, Serialize, Deserialize)]
pub enum RunStatus {
    Continue,
    Failed,
    Pending,
    Done,
}

/// Map of step names to prompts used when running each step.
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptPlan {
    pub prompts: BTreeMap<String, String>,
}

/// Mutable context carried through the flow: current step, answers, prompts, model, and key-value context.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub step: Option<Step>,
    pub answers: Vec<String>,
    pub prompt_plan: PromptPlan,
    pub model: Model,
    pub model_key: Option<String>,
    pub context: BTreeMap<String, String>,
}

impl Default for ExecutionContext {
    /// Builds a default execution context with no step, empty answers/prompts, and default model.
    fn default() -> Self {
        Self {
            step: None,
            answers: vec![],
            prompt_plan: PromptPlan {
                prompts: BTreeMap::new(),
            },
            model: Model::GPT_5POINT1,
            model_key: Some("<api_key>".to_string()),
            context: BTreeMap::new(),
        }
    }
}

/// Contract interface for the step agent: run a flow, resume, get/attach context.
trait StepAgent {
    fn new(mcp_contract_addresses: Vec<String>) -> Result<Self, String>
    where
        Self: Sized;
    async fn run(&self, namespace: String, flow_id: String, execution_context: ExecutionContext) -> (RunStatus, ExecutionContext);
    async fn resume(&self, namespace: String, flow_id: String) -> (RunStatus, ExecutionContext);
    async fn get_context(&self, namespace: String, flow_id: String) -> ExecutionContext;
    async fn attach_context(&mut self, namespace: String, flow_id: String, key: String, value: String) -> Result<(), String>;
}

/// Persistent state for the step-agent contract: MCP addresses and step handlers.
#[derive(Serialize, Deserialize)]
pub struct StepAgentContractState {
    mcp_contract_addresses: Vec<String>,
    handlers: BTreeMap<Step, Box<dyn State>>,
}

impl weil_rs::traits::WeilType for StepAgentContractState {}

#[smart_contract]
impl StepAgent for StepAgentContractState {
    /// Creates a new step agent with the given MCP addresses and registers handlers for steps A–E.
    ///
    /// # Arguments
    /// * `mcp_contract_addresses` - Contract IDs of MCPs used by step handlers.
    ///
    /// # Returns
    /// `Ok(Self)` on success, or `Err(String)` if construction fails.
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

    /// Runs the flow to completion (or until Pending/Failed) using the provided execution context.
    ///
    /// Dispatches each step to the corresponding handler; handlers may return `Continue`,
    /// `Pending`, `Done`, or `Failed`. Context is updated in place.
    ///
    /// # Arguments
    /// * `namespace` - Flow namespace (e.g. for flow registry).
    /// * `flow_id` - Flow identifier.
    /// * `execution_context` - Initial context (step, prompt_plan, model, etc.).
    ///
    /// # Returns
    /// `(RunStatus, ExecutionContext)` — final status and possibly updated context.
    #[query]
    async fn run(
        &self,
        namespace: String,
        flow_id: String,
        execution_context: ExecutionContext,
    ) -> (RunStatus, ExecutionContext) {
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
            let (step, status) = handler.do_work(self.mcp_contract_addresses.clone(), &mut context);
            context.step = step;

            match status {
                RunStatus::Continue => {
                    // do the next flow
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

    /// Resumes a previously started flow by loading its execution context from the flow registry.
    ///
    /// # Arguments
    /// * `namespace` - Flow namespace.
    /// * `flow_id` - Flow identifier.
    ///
    /// # Returns
    /// `(RunStatus, ExecutionContext)` from running the loaded context (panics if context is missing).
    #[query]
    async fn resume(&self, namespace: String, flow_id: String) -> (RunStatus, ExecutionContext) {
        let flow_registry_address = Runtime::contract_id_for_name(FLOW_REGISTRY_NAME).unwrap();
        let flow_registry = FlowRegistry::new(flow_registry_address);

        let serialized_ctx = flow_registry
            .get_execution_context(namespace.clone(), flow_id.clone())
            .unwrap();
        // we MUST get a context to proceed
        let ctx = serialized_ctx.unwrap();

        let context = serde_json::from_str::<ExecutionContext>(&ctx).unwrap();

        self.run(namespace, flow_id, context).await
    }

    /// Fetches the stored execution context for the given namespace and flow_id from the flow registry.
    ///
    /// # Arguments
    /// * `namespace` - Flow namespace.
    /// * `flow_id` - Flow identifier.
    ///
    /// # Returns
    /// The deserialized `ExecutionContext` (panics if missing or invalid).
    #[query]
    async fn get_context(&self, namespace: String, flow_id: String) -> ExecutionContext {
        let flow_registry_address = Runtime::contract_id_for_name(FLOW_REGISTRY_NAME).unwrap();
        let flow_registry = FlowRegistry::new(flow_registry_address);

        let serialized_ctx = flow_registry
            .get_execution_context(namespace.clone(), flow_id.clone())
            .unwrap();
        // we MUST get a context to proceed
        let ctx = serialized_ctx.unwrap();

        let context = serde_json::from_str::<ExecutionContext>(&ctx).unwrap();

        context
    }

    /// Attaches a key-value pair to the execution context for the given flow, then persists it.
    ///
    /// Fails if no context exists or if the context map has reached `MAX_CONTEXT_SIZE`.
    ///
    /// # Arguments
    /// * `namespace` - Flow namespace.
    /// * `flow_id` - Flow identifier.
    /// * `key` - Context key to set.
    /// * `value` - Value to set.
    ///
    /// # Errors
    /// Returns `Err(String)` if context is missing or context size limit is reached.
    #[mutate]
    async fn attach_context(&mut self, namespace: String, flow_id: String, key: String, value: String) -> Result<(), String> {
        let flow_registry_address = Runtime::contract_id_for_name(FLOW_REGISTRY_NAME).unwrap();
        let flow_registry = FlowRegistry::new(flow_registry_address);

        let serialized_ctx = flow_registry
            .get_execution_context(namespace.clone(), flow_id.clone())
            .map_err(|e| e.to_string());
        // we MUST get a context to proceed
        let ctx = serialized_ctx.unwrap();

        if ctx.is_none() {
            return Err("No execution context found".to_string());
        }
        let ctx = ctx.unwrap();

        let mut exec_context = serde_json::from_str::<ExecutionContext>(&ctx).unwrap();

        // get the size of the context map
        let context_size = exec_context.context.len();
        if context_size >= MAX_CONTEXT_SIZE {
            return Err("Context size limit reached".to_string());
        }
   
        exec_context.context.insert(key, value);

        let updated_serialized_ctx = serde_json::to_string(&exec_context).unwrap();

        flow_registry.persist_execution_context(namespace, flow_id, updated_serialized_ctx).unwrap();

        Ok(())
    }
}

/// Step A handler: runs the "A" prompt (if present) via base agent, then continues to step B.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateA;

#[typetag::serde]
impl State for StateA {
    fn name(&self) -> &'static str {
        "A"
    }

    /// Runs prompt "A" via base agent helper, appends answer to context; returns next step B and Continue.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus) {

       if let Some(prompt) = ctx.prompt_plan.prompts.get(self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
                ctx.model_key.clone()
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (None, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

       println!("ran step A");

        // Decide next step explicitly via Continue(Some(...))
        (Some(Step::B), RunStatus::Continue)
    }
}

/// Step B handler: runs the "B" prompt via base agent, then returns Pending with next step C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateB;

#[typetag::serde]
impl State for StateB {
    fn name(&self) -> &'static str {
        "B"
    }

    /// Runs prompt "B" via base agent, sets next step C, returns Pending so execution can be resumed later.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus) {
       if let Some(prompt) = ctx.prompt_plan.prompts.get(self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
                ctx.model_key.clone()
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (None, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step B");
        // Move to next step after completing work
        ctx.step = Some(Step::C);

        (Some(Step::C), RunStatus::Pending)
    }
}

/// Step C handler: branches on context "datasource" (e.g. aurora/snowflake), then advances to step D.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateC;

#[typetag::serde]
impl State for StateC {
    fn name(&self) -> &'static str {
        "C"
    }

    /// Reads "datasource" from context and performs datasource-specific work; sets next step D and returns Pending.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus) {

        let context=  &ctx.context;
        if let Some(source) = context.get("datasource") {
            match source.as_str() {
                "aurora" => {
                    // do something specific for aurora
                    println!("found datasource: aurora, doing aurora-specific work");
                },
                "snowflake" => {
                    // do something specific for snowflake
                    println!("found datasource: snowflake, doing snowflake-specific work");
                },
                _ => {
                    println!("found datasource: {}, but no specific handler for it", source);
                }
            }
        } else {
            println!("no datasource in context");
        }
        println!("ran step C");
        ctx.step = Some(Step::D);

        (Some(Step::D), RunStatus::Pending)
    }
}

/// Step D handler: runs the "D" prompt via base agent, then advances to step E with Pending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateD;

#[typetag::serde]
impl State for StateD {
    fn name(&self) -> &'static str {
        "D"
    }

    /// Runs prompt "D" via base agent, sets next step E, returns Pending.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus) {
       if let Some(prompt) = ctx.prompt_plan.prompts.get(self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
                ctx.model_key.clone()
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (None, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step D");
        ctx.step = Some(Step::E);

        (Some(Step::E), RunStatus::Pending)
    }
}

/// Step E handler: runs the "E" prompt via base agent, then marks the flow Done (terminal step).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateE;

#[typetag::serde]
impl State for StateE {
    fn name(&self) -> &'static str {
        "E"
    }

    /// Runs prompt "E" via base agent, clears step (None), returns Done to end the flow.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus) {
       if let Some(prompt) = ctx.prompt_plan.prompts.get(self.name()){
            let base_agent_helper_address = Runtime::contract_id_for_name(BASE_AGENT_HELPER_NAME).unwrap();
            let base_agent_helper = BaseAgentHelper::new(base_agent_helper_address);

            let res = match base_agent_helper.run_task(
                prompt.into(),
                mcp_addresses[0].clone(), //or any corresponding mcp for this task
                ctx.model.clone(),
                ctx.model_key.clone()
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (None, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step E");
        ctx.step = None; // terminal

        (None, RunStatus::Done)
    }
}

