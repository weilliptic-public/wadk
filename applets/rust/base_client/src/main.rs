use serde::{Deserialize, Serialize};
use weil_rs::errors::WeilError;
use weil_wallet::{contract::ContractId, streaming::ByteStream, wallet::{PrivateKey, Wallet}, WeilClient, WeilContractClient};

struct BaseAgentClient {
    client: WeilContractClient,
}

impl BaseAgentClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(BaseAgentClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    pub async fn run_task(&self, task_prompt: String) -> Result<String, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            task_prompt: String,
        }

        let args = Args { task_prompt };

        let resp = self.client.execute("run_task".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<String>(&result)?;

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

    let client = BaseAgentClient::new(contract_id, wallet).unwrap();
    let task_prompt = "<YOUR_PROMPT_HERE>";
    let data = client.run_task(task_prompt.to_string()).await;
}
