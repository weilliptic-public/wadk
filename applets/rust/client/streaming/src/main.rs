use futures_util::StreamExt;
use serde::Serialize;
use std::io;
use std::io::Write;
use weil_wallet::{
    contract::ContractId,
    streaming::ByteStream,
    wallet::{PrivateKey, Wallet},
    WeilClient, WeilContractClient,
};

struct StreamingClient {
    client: WeilContractClient,
}

impl StreamingClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(StreamingClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
        })
    }

    pub async fn health_check(&self, prompt: String) -> Result<ByteStream, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            prompt: String,
        }

        let args = Args { prompt };

        let resp = self
            .client
            .execute_with_streaming(
                "health_check".to_string(),
                serde_json::to_string(&args).unwrap(),
            )
            .await?;

        Ok(resp)
    }
}

#[tokio::main]
async fn main() {
    let private_key = PrivateKey::from_file("/root/.weilliptic/private_key.wc").unwrap();
    let wallet = Wallet::new(private_key).unwrap();

    // put your contract id here!
    let contract_id = "00000002d011ad7c20eed92cc30811c86e5da68e832619d3fb5e82834efb99e0562d9f3f"
        .parse::<ContractId>()
        .unwrap();

    let client = StreamingClient::new(contract_id, wallet).unwrap();

    let mut res = client
        .health_check("Why is sky blue ?".to_string())
        .await
        .unwrap();

    let mut stdout = io::stdout();

    while let Some(chunk) = res.next().await {
        print!("{}", String::from_utf8(chunk.into()).unwrap());
        stdout.flush().unwrap();
    }

    println!("\n");
}
