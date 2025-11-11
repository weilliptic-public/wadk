use serde::Serialize;
use weil_rs::errors::WeilError;
use weil_wallet::{
    contract::ContractId,
    wallet::{PrivateKey, Wallet},
    WeilClient, WeilContractClient,
};

struct YutakaClient {
    client: WeilContractClient,
}

impl YutakaClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(YutakaClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    pub async fn name(&self) -> Result<String, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {}

        let args = Args {};

        let resp = self
            .client
            .execute("name".to_string(), serde_json::to_string(&args).unwrap())
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<String>(&result)?;

        Ok(result)
    }

    pub async fn symbol(&self) -> Result<String, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {}

        let args = Args {};

        let resp = self
            .client
            .execute("symbol".to_string(), serde_json::to_string(&args).unwrap())
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<String>(&result)?;

        Ok(result)
    }

    pub async fn decimals(&self) -> Result<u8, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {}

        let args = Args {};

        let resp = self
            .client
            .execute(
                "decimals".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<u8>(&result)?;

        Ok(result)
    }

    pub async fn details(&self) -> Result<(String, String, u8), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {}

        let args = Args {};

        let resp = self
            .client
            .execute("details".to_string(), serde_json::to_string(&args).unwrap())
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<(String, String, u8)>(&result)?;

        Ok(result)
    }

    pub async fn total_supply(&self) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {}

        let args = Args {};

        let resp = self
            .client
            .execute(
                "total_supply".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    pub async fn balance_for(&self, addr: String) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            addr: String,
        }

        let args = Args { addr };

        let resp = self
            .client
            .execute(
                "balance_for".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    pub async fn transfer(&self, to_addr: String, amount: u64) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            to_addr: String,
            amount: u64,
        }

        let args = Args { to_addr, amount };

        let resp = self
            .client
            .execute(
                "transfer".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    pub async fn approve(&self, spender: String, amount: u64) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            spender: String,
            amount: u64,
        }

        let args = Args { spender, amount };

        let resp = self
            .client
            .execute("approve".to_string(), serde_json::to_string(&args).unwrap())
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    pub async fn transfer_from(
        &self,
        from_addr: String,
        to_addr: String,
        amount: u64,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            from_addr: String,
            to_addr: String,
            amount: u64,
        }

        let args = Args {
            from_addr,
            to_addr,
            amount,
        };

        let resp = self
            .client
            .execute(
                "transfer_from".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<()>(&result)?;

        Ok(result)
    }

    pub async fn allowance(&self, owner: String, spender: String) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            owner: String,
            spender: String,
        }

        let args = Args { owner, spender };

        let resp = self
            .client
            .execute(
                "allowance".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;
        let result = txn_result?;
        let result = serde_json::from_str::<u64>(&result)?;

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

    let client = YutakaClient::new(contract_id, wallet).unwrap();

    let details = client.details().await;

    println!("details of Yutaka is: {:?}", details);

    let balance_for = client.balance_for("Avinash".to_string()).await;

    println!("balance for Avinash is: {:?}", balance_for);

    let _ = client.transfer("Avinash".to_string(), 200).await;

    let balance_for = client.balance_for("Avinash".to_string()).await;

    println!("balance for Avinash is: {:?}", balance_for);
}
