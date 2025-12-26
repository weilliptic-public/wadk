use serde::{Deserialize, Serialize};
use weil_rs::errors::WeilError;
use weil_wallet::{contract::ContractId, streaming::ByteStream, wallet::{PrivateKey, Wallet}, WeilClient, WeilContractClient};

struct MultiAgentClient {
    client: WeilContractClient,
}

impl MultiAgentClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(MultiAgentClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    pub async fn run_tasks(&self, task_descriptions: Vec<String>) -> Result<String, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            task_descriptions: Vec<String>,
        }

        let args = Args { task_descriptions };

        let resp = self.client.execute("run_tasks".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<String>(&result)?;

        Ok(result)
    }

    pub async fn update_resume_task_index(&self, index: u32, previous_task_result: Option<String>) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            index: u32,
            previous_task_result: Option<String>,
        }

        let args = Args { index, previous_task_result };

        let resp = self.client.execute("update_resume_task_index".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

}

#[tokio::main]
async fn main() {
    let private_key = PrivateKey::from_file("/root/.weilliptic/private_key.wc").unwrap();
    let wallet = Wallet::new(private_key).unwrap();

    // put your contract id here!
    let contract_id = "00000002ef5e2433d9ffd69f0413622bae0fb3a3db12720a837e88874717d24a478d16ee"
        .parse::<ContractId>()
        .unwrap();

    let client = MultiAgentClient::new(contract_id, wallet).unwrap();
    let task_prompts = vec!["<YOUR_FIRST_PROMPT>".into(), "<YOUR_SECOND_PROMPT>".into()];
    let data = client.run_tasks(task_prompts).await;
}