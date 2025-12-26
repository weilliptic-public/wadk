use std::collections::BTreeMap;
use weil_wallet::{contract::ContractId, wallet::{PrivateKey, Wallet}};

mod client;
mod registry;

use client::*;

#[tokio::main]
async fn main(){
    // replace with the actual contract id of the deployed StepAgent contract
    let step_agent_contract_id = "aaaaaas2errvd2ht245qxh5cvs7j5jksnyrxmigbdut6fkbz7s7zfzvpvm"
        .parse::<ContractId>().unwrap();
    // replace with the actual location of your private key file
    let private_key = PrivateKey::from_file("./private_key.wc").unwrap();
    let wallet = Wallet::new(private_key.clone()).unwrap();
    let step_client = StepAgentClient::new(step_agent_contract_id, wallet.clone()).unwrap();

    let namespace = "a".to_string();
    let flow_id = "b".to_string();

    let initial_prompts = BTreeMap::new();

    let initial_execution_context = ExecutionContext {
        step: Some(Step::A),
        answers: vec![],
        prompt_plan: PromptPlan {
            prompts: initial_prompts,
        },
        model: Model::QWEN_235B,
    };

    let result = step_client.run(namespace.clone(), flow_id.clone(), initial_execution_context).await.unwrap();
    println!("{:?}", result.0);
    let resume_result = step_client.resume(namespace, flow_id).await.unwrap();
    println!("{:?}", resume_result.0);
}
