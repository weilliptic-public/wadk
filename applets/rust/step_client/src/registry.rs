use serde::Serialize;
use wadk_utils::errors::WeilError;
use weil_wallet::{contract::ContractId, wallet::Wallet, WeilClient, WeilContractClient};

pub struct FlowRegistryClient {
    client: WeilContractClient,
}

impl FlowRegistryClient {
    /// Creates a new FlowRegistryClient instance
    /// 
    /// # Arguments
    /// * `contract_id` - The contract ID of the flow registry
    /// * `wallet` - The wallet to use for signing transactions
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(FlowRegistryClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    /// Retrieves a stored execution context from the flow registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the context is stored
    /// * `flow_id` - The unique identifier of the flow
    /// 
    /// # Returns
    /// An optional JSON string representing the execution context
    pub async fn get_execution_context(&self, namespace: String, flow_id: String) -> Result<Option<String>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        let resp = self.client.execute("get_execution_context".to_string(), serde_json::to_string(&args).unwrap()).await?;

        println!("response : {:?}", resp);
        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<Option<String>>(&result)?;

        Ok(result)
    }

    /// Stores an execution context in the flow registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the context should be stored
    /// * `flow_id` - The unique identifier of the flow
    /// * `ctx` - The JSON string representation of the execution context
    pub async fn persist_execution_context(&self, namespace: String, flow_id: String, ctx: String) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
            ctx: String,
        }

        let args = Args { namespace, flow_id, ctx };

        let resp = self.client.execute("persist_execution_context".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    /// Deletes a stored execution context from the flow registry
    /// 
    /// # Arguments
    /// * `namespace` - The namespace where the context is stored
    /// * `flow_id` - The unique identifier of the flow to delete
    pub async fn delete_context(&self, namespace: String, flow_id: String) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        let resp = self.client.execute("delete_context".to_string(), serde_json::to_string(&args).unwrap()).await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

}