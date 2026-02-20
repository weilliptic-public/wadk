
use serde::{Deserialize, Serialize};
use weil_macros::{constructor, query, smart_contract, WeilType};
use weil_rs::ai::agents::{Model, base::BaseAgentHelper, flow_registry::FlowRegistry};
use std::collections::BTreeMap;
use weil_rs::runtime::Runtime;

const BASE_AGENT_HELPER_NAME: &str = "base_agent_helper::weil";
const FLOW_REGISTRY_NAME: &str = "flow_registry::weil";

#[typetag::serde(tag = "type")]
pub(crate) trait State: Send + Sync {
    fn name(&self) -> &'static str;
    /// Takes ownership of ctx, returns updated ctx + status.
    fn do_work(
        &self,
        mcp_addresses: Vec<String>,
        ctx: &mut ExecutionContext,
    ) -> (Option<Step>, RunStatus);
}


#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Hash, WeilType)]
pub enum Step {
    A,
    B,
    C,
    D,
    E,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RunStatus {
    Continue,
    Failed,
    Pending,
    Done,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptPlan {
    pub prompts: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub step: Option<Step>,
    pub answers: Vec<String>,
    pub prompt_plan: PromptPlan,
    pub model: Model,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            step: None,
            answers: vec![],
            prompt_plan: PromptPlan {
                prompts: BTreeMap::new(),
            },
            model: Model::QWEN_235B,
        }
    }
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
    handlers: BTreeMap<Step, Box<dyn State>>,
}

impl weil_rs::traits::WeilType for StepAgentContractState {}

#[smart_contract]
impl StepAgent for StepAgentContractState {
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
    }}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateA;

#[typetag::serde]
impl State for StateA {
    fn name(&self) -> &'static str {
        "A"
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateB;

#[typetag::serde]
impl State for StateB {
    fn name(&self) -> &'static str {
        "B"
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateC;

#[typetag::serde]
impl State for StateC {
    fn name(&self) -> &'static str {
        "C"
    }

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
            ).map_err(|e| e.to_string()) {
                Ok(r) => r,
                Err(_) => return (None, RunStatus::Failed),
            };

            ctx.answers.push(res);
       };

        println!("ran step C");
        ctx.step = Some(Step::D);

        (Some(Step::D), RunStatus::Pending)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateD;

#[typetag::serde]
impl State for StateD {
    fn name(&self) -> &'static str {
        "D"
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateE;

#[typetag::serde]
impl State for StateE {
    fn name(&self) -> &'static str {
        "E"
    }

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

