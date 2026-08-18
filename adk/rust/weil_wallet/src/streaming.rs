use bytes::Bytes;
use futures_util::Stream;
use std::pin::Pin;

/// Represents the streaming response from the `WeilChain` platform.
/// This is the return type of the `execute_with_streaming` methods of the clients i.e `WeilClient` and `WeilContractClient`.
///
/// <br>
///
/// # Example
/// Below is a complete example of the client code for calling a `ask_llm` exported method of a contract which returns a `ByteStream` response.
/// It shows how the response is processed using `next` on the `ByteStream` and interpreting raw bytes i.e `Vec<u8>` as `UTF-8 encoded` string.
/// ```
/// use futures_util::StreamExt;
/// use serde::Serialize;
/// use std::io;
/// use std::io::Write;
/// use weil_wallet::{
///     contract::ContractId,
///     streaming::ByteStream,
///     wallet::{PrivateKey, Wallet},
///    WeilClient, WeilContractClient,
/// };
///
/// struct StreamingClient {
///     client: WeilContractClient,
/// }
///
/// impl StreamingClient {
///     pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
///         Ok(StreamingClient {
///             client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
///         })
///     }
///
///     pub async fn ask_llm(&self, prompt: String) -> Result<ByteStream, anyhow::Error> {
///         #[derive(Serialize)]
///         struct Args {
///             prompt: String,
///         }
///
///         let args = Args { prompt };
///
///         let resp = self
///             .client
///             .execute_with_streaming(
///                 "ask_llm".to_string(),
///                 serde_json::to_string(&args).unwrap(),
///             )
///             .await?;
///
///         Ok(resp)
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let private_key = PrivateKey::from_file("/root/.weilliptic/private_key.wc").unwrap();
///     let wallet = Wallet::new(private_key).unwrap();
///
///     // put your contract id here!
///     let contract_id = "00000002d011ad7c20eed92cc30811c86e5da68e832619d3fb5e82834efb99e0562d9f3f"
///         .parse::<ContractId>()
///         .unwrap();
///
///     let client = StreamingClient::new(contract_id, wallet).unwrap();
///
///     let mut res = client
///         .ask_llm("Why is sky blue ?".to_string())
///         .await
///         .unwrap();
///
///     let mut stdout = io::stdout();
///
///     while let Some(chunk) = res.next().await {
///         print!("{}", String::from_utf8(chunk.into()).unwrap());
///         stdout.flush().unwrap();
///     }
///
///     println!("\n");
/// }
/// ```
pub struct ByteStream {
    stream: Box<dyn Stream<Item = reqwest::Result<Bytes>>>,
}

impl ByteStream {
    pub(crate) fn new<T: Stream<Item = reqwest::Result<Bytes>> + 'static>(stream: T) -> Self {
        ByteStream {
            stream: Box::new(stream),
        }
    }
}

impl Stream for ByteStream {
    type Item = Vec<u8>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let s = &mut *self.as_mut().stream;
        let pinned_stream = unsafe { Pin::new_unchecked(s) };

        let std::task::Poll::Ready(item) = pinned_stream.poll_next(cx) else {
            return std::task::Poll::Pending;
        };

        let Some(res) = item else {
            return std::task::Poll::Ready(None);
        };

        match res {
            Ok(ok_val) => std::task::Poll::Ready(Some(ok_val.to_vec())),
            Err(_) => std::task::Poll::Ready(None),
        }
    }
}
