use crate::{contract::ContractId, wallet::Wallet, WeilClient, WeilContractClient};
use serde::Serialize;
use weil_rs::errors::WeilError;

pub struct FlowRegistryClient {
    client: WeilContractClient,
}

impl FlowRegistryClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(FlowRegistryClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    pub async fn get_execution_context(
        &self,
        namespace: String,
        flow_id: String,
    ) -> Result<Option<String>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        let resp = self
            .client
            .execute(
                "get_execution_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<Option<String>>(&result)?;

        Ok(result)
    }

    pub async fn persist_execution_context(
        &self,
        namespace: String,
        flow_id: String,
        ctx: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
            ctx: String,
        }

        let args = Args {
            namespace,
            flow_id,
            ctx,
        };

        let resp = self
            .client
            .execute(
                "persist_execution_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    pub async fn delete_context(
        &self,
        namespace: String,
        flow_id: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            namespace: String,
            flow_id: String,
        }

        let args = Args { namespace, flow_id };

        let resp = self
            .client
            .execute(
                "delete_context".to_string(),
                serde_json::to_string(&args).unwrap(),
                None,
                None,
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }
}
