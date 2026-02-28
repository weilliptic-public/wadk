use crate::{
    constants::SENTINEL_HOST, streaming::ByteStream, transaction::TransactionResult,
    utils::compress,
};
use request::SubmitTxnRequest;
use reqwest::{multipart, Client, Response};

pub mod request;

pub(crate) struct PlatformApi;

impl PlatformApi {
    async fn submit_transaction_inner(
        payload: SubmitTxnRequest,
        client: Client,
        is_non_blocking: bool,
    ) -> Result<Response, anyhow::Error> {
        let tx_payload = compress(&payload).map_err(|err| {
            anyhow::Error::msg(format!("payload compression failed: {}", err.to_string()))
        })?;

        let form = multipart::Form::new().part(
            "transaction",
            multipart::Part::bytes(tx_payload)
                .file_name("transaction_data")
                .mime_str("application/octet-stream")
                .unwrap(),
        );

        let mut request = client
            .post(format!("{}/contracts/execute_smartcontract", SENTINEL_HOST))
            .multipart(form);

        if is_non_blocking {
            request = request.header("x-non-blocking", "true");
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::Error::msg("failed to submit the transaction"));
        }

        Ok(response)
    }

    pub async fn submit_transaction(
        payload: SubmitTxnRequest,
        client: Client,
        is_non_blocking: bool,
    ) -> anyhow::Result<TransactionResult> {
        let response =
            PlatformApi::submit_transaction_inner(payload, client, is_non_blocking).await?;

        if is_non_blocking {
            return Ok(TransactionResult::default());
        }

        let result = response.json::<TransactionResult>().await?;

        Ok(result)
    }

    pub async fn submit_transaction_with_streaming(
        payload: SubmitTxnRequest,
        client: Client,
    ) -> anyhow::Result<ByteStream> {
        let response = PlatformApi::submit_transaction_inner(payload, client, false).await?;
        let stream = response.bytes_stream();

        Ok(ByteStream::new(stream))
    }
}
