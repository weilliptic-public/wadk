use serde::{Deserialize, Serialize};
use weil_rs::errors::WeilError;
use weil_wallet::{
    WeilClient, WeilContractClient,
    contract::ContractId,
    transaction::{TransactionResult, TransactionStatus},
    wallet::{S3Credentials as WalletS3Credentials, Wallet},
};
use weilstream_core::{PublishReceipt, RangeProof, StreamBatch, StreamRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket_name: String,
    pub region: String,
}

fn to_wallet_s3_credentials(
    creds: Option<S3Credentials>,
) -> Result<Option<WalletS3Credentials>, anyhow::Error> {
    creds
        .map(|creds| {
            let value = serde_json::to_value(creds)?;
            Ok(serde_json::from_value(value)?)
        })
        .transpose()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PayloadRef {
    pub payload_hash: String,
    pub object_location: String,
    pub content_len: u64,
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Payload {
    Inline(String),
    Reference(PayloadRef),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub topic: String,
    pub sequence: u64,
    pub owner: String,
    pub commits: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitReceipt {
    pub topic: String,
    pub sequence: u64,
    pub previous_sequence: Option<u64>,
    pub committed_by: String,
    pub txn_id: String,
    pub block_height: u64,
    pub block_timestamp: String,
}

/// The application payload of a settled call, or an error naming why there isn't one.
///
/// The transaction's own status is checked before its result is parsed, and that ordering is
/// the whole point. A call that has not executed yet (`InProgress`) or was rejected
/// (`Failed`) comes back with an **empty** `txn_result`, so parsing it first yields
/// "EOF while parsing a value at line 1 column 0" — an error about JSON that says nothing
/// about what actually happened, and sends the reader looking for a serialization bug that
/// does not exist.
///
/// `InProgress` is worth telling apart from `Failed`: it means the transaction was accepted
/// but has not been executed yet, which is a retriable condition rather than a rejection.
fn settled_body(resp: TransactionResult) -> Result<String, anyhow::Error> {
    match resp.status {
        TransactionStatus::Confirmed | TransactionStatus::Finalized => {}
        TransactionStatus::InProgress => anyhow::bail!(
            "the transaction was accepted but has not executed yet — no result to read"
        ),
        TransactionStatus::Failed => anyhow::bail!(
            "the transaction failed{}",
            if resp.txn_result.is_empty() {
                String::new()
            } else {
                format!(": {}", resp.txn_result)
            }
        ),
    }

    let txn_result = serde_json::from_str::<Result<String, WeilError>>(&resp.txn_result)?;

    Ok(txn_result?)
}

pub(crate) struct WeilStreamConsumerClient {
    client: WeilContractClient,
    is_remote: bool,
}

impl WeilStreamConsumerClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamConsumerClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
            is_remote: false,
        })
    }

    pub async fn with_api_key(
        contract_id: ContractId,
        api_key: String,
        creds: Option<S3Credentials>,
    ) -> Result<Self, anyhow::Error> {
        let creds = to_wallet_s3_credentials(creds)?;

        Ok(WeilStreamConsumerClient {
            client: WeilClient::with_api_key(&api_key, creds, None, None).await?
                .to_contract_client(contract_id),
            is_remote: true,
        })
    }

    async fn execute(
        &self,
        method_name: &str,
        method_args: String,
        is_non_blocking: Option<bool>,
    ) -> Result<TransactionResult, anyhow::Error> {
        if self.is_remote {
            self.client
                .execute_remotely(method_name.to_string(), method_args, None, is_non_blocking)
                .await
        } else {
            self.client
                .execute(method_name.to_string(), method_args, None, is_non_blocking)
                .await
        }
    }

    pub async fn read_batch(
        &self,
        topic: &str,
        from_seq: u64,
        max_count: u32,
    ) -> Result<Vec<StreamRecord>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
            from_seq: u64,
            max_count: u32,
        }

        let args = Args {
            topic,
            from_seq,
            max_count,
        };

        let resp = self
            .execute("read_batch", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Vec<StreamRecord>>(&result)?;

        Ok(result)
    }

    /// One host round trip returns one segment block's records for
    /// `topic` starting at `from_seq`
    pub async fn read_block(
        &self,
        topic: &str,
        from_seq: u64,
    ) -> Result<StreamBatch, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
            from_seq: u64,
        }

        let args = Args { topic, from_seq };

        let resp = self
            .execute("read_block", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let batch = serde_json::from_str::<StreamBatch>(&result)?;

        Ok(batch)
    }

    pub async fn read_seqs(
        &self,
        topic: String,
        seqs: Vec<u64>,
    ) -> Result<Vec<StreamRecord>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            seqs: Vec<u64>,
        }

        let args = Args { topic, seqs };

        let resp = self
            .execute("read_seqs", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Vec<StreamRecord>>(&result)?;

        Ok(result)
    }

    pub async fn read_seq(
        &self,
        topic: String,
        seq: u64,
    ) -> Result<Option<StreamRecord>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            seq: u64,
        }

        let args = Args { topic, seq };

        let resp = self
            .execute("read_seq", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Option<StreamRecord>>(&result)?;

        Ok(result)
    }

    /// The topic's beginning offset — Kafka's `beginningOffsets`.
    pub async fn beginning_offset(&self, topic: &str) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
        }

        let args = Args { topic };

        let resp = self
            .execute(
                "beginning_offset",
                serde_json::to_string(&args).unwrap(),
                None,
            )
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    /// The topic's end offset — Kafka's `endOffsets`.
    pub async fn end_offset(&self, topic: &str) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
        }

        let args = Args { topic };

        let resp = self
            .execute("end_offset", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    pub async fn get_committed(&self, topic: String) -> Result<Option<u64>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
        }

        let args = Args { topic };

        let resp = self
            .execute("get_committed", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Option<u64>>(&result)?;

        Ok(result)
    }

    pub async fn checkpoint(&self, topic: &str) -> Result<Option<Checkpoint>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
        }

        let args = Args { topic };

        let resp = self
            .execute("checkpoint", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Option<Checkpoint>>(&result)?;

        Ok(result)
    }

    pub async fn commit(&self, topic: &str, sequence: u64) -> Result<CommitReceipt, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
            sequence: u64,
        }

        let args = Args { topic, sequence };

        let resp = self
            .execute("commit", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<CommitReceipt>(&result)?;

        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopicInfo {
    pub topic: String,
    pub next_seq: u64,
    pub low_water_seq: u64,
    pub head_entry_hash: String,
    pub created_at: String,
}

pub(crate) struct WeilStreamPublisherClient {
    client: WeilContractClient,
    is_remote: bool,
}

impl WeilStreamPublisherClient {
    pub fn new(contract_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamPublisherClient {
            client: WeilClient::new(wallet, None)?.to_contract_client(contract_id),
            is_remote: false,
        })
    }

    pub async fn with_apikey(
        contract_id: ContractId,
        api_key: String,
        creds: Option<S3Credentials>,
    ) -> Result<Self, anyhow::Error> {
        let creds = to_wallet_s3_credentials(creds)?;

        Ok(WeilStreamPublisherClient {
            client: WeilClient::with_api_key(&api_key, creds, None, None).await?
                .to_contract_client(contract_id),
            is_remote: true,
        })
    }

    async fn execute(
        &self,
        method_name: &str,
        method_args: String,
        is_non_blocking: Option<bool>,
    ) -> Result<TransactionResult, anyhow::Error> {
        if self.is_remote {
            self.client
                .execute_remotely(method_name.to_string(), method_args, None, is_non_blocking)
                .await
        } else {
            self.client
                .execute(method_name.to_string(), method_args, None, is_non_blocking)
                .await
        }
    }

    /// Publish an event and wait for the chain to execute it. The receipt
    /// carries the assigned sequence, txn id and block info.
    ///
    /// Callers that don't want to wait should use [`Self::publish_async`]
    /// instead -- that's the sole reason both methods exist, so `publish`
    /// no longer takes a mode argument.
    pub async fn publish(
        &self,
        topic: &str,
        payload: String,
    ) -> Result<PublishReceipt, anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
            payload: String,
        }

        let args = Args { topic, payload };

        let resp = self
            .execute("publish", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<PublishReceipt>(&result)?;

        Ok(result)
    }

    pub async fn publish_async(
        &self,
        topic: &str,
        payload: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args<'a> {
            topic: &'a str,
            payload: String,
        }

        let args = Args { topic, payload };

        let resp = self
            .execute("publish", serde_json::to_string(&args).unwrap(), Some(true))
            .await?;

        let _ = resp;
        Ok(())
    }

    pub async fn publish_ref(
        &self,
        topic: String,
        payload: PayloadRef,
    ) -> Result<PublishReceipt, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            payload: PayloadRef,
        }

        let args = Args { topic, payload };

        let resp = self
            .execute("publish_ref", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<PublishReceipt>(&result)?;

        Ok(result)
    }

    pub async fn create_topic(
        &self,
        topic: String,
        description: String,
        retention_in_bytes: u64,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            description: String,
            retention_in_bytes: u64,
        }

        let args = Args {
            topic,
            description,
            retention_in_bytes,
        };

        let _ = self
            .execute("create_topic", serde_json::to_string(&args).unwrap(), None)
            .await?;

        Ok(())
    }

    pub async fn next_seq(&self, topic: String) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
        }

        let args = Args { topic };

        let resp = self
            .execute("next_seq", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    pub async fn topic_info(&self, topic: String) -> Result<TopicInfo, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
        }

        let args = Args { topic };

        let resp = self
            .execute("topic_info", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<TopicInfo>(&result)?;

        Ok(result)
    }

    pub async fn verify(
        &self,
        topic: String,
        seq: u64,
        payload_hash: String,
    ) -> Result<bool, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            seq: u64,
            payload_hash: String,
        }

        let args = Args {
            topic,
            seq,
            payload_hash,
        };

        let resp = self
            .execute("verify", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<bool>(&result)?;

        Ok(result)
    }

    pub async fn verify_range(
        &self,
        topic: String,
        from_seq: u64,
        to_seq: u64,
        expected_range_head: String,
    ) -> Result<bool, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            from_seq: u64,
            to_seq: u64,
            expected_range_head: String,
        }

        let args = Args {
            topic,
            from_seq,
            to_seq,
            expected_range_head,
        };

        let resp = self
            .execute("verify_range", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<bool>(&result)?;

        Ok(result)
    }

    pub async fn proof_for_range(
        &self,
        topic: String,
        from_seq: u64,
        to_seq: u64,
    ) -> Result<RangeProof, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            from_seq: u64,
            to_seq: u64,
        }

        let args = Args {
            topic,
            from_seq,
            to_seq,
        };

        let resp = self
            .execute(
                "proof_for_range",
                serde_json::to_string(&args).unwrap(),
                None,
            )
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<RangeProof>(&result)?;

        Ok(result)
    }

    pub async fn trim(&self, topic: String, up_to_seq: u64) -> Result<u64, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            up_to_seq: u64,
        }

        let args = Args { topic, up_to_seq };

        let resp = self
            .execute("trim", serde_json::to_string(&args).unwrap(), None)
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<u64>(&result)?;

        Ok(result)
    }

    pub async fn read_batch_data(
        &self,
        topic: String,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<StreamRecord>, anyhow::Error> {
        #[derive(Serialize)]
        struct Args {
            topic: String,
            offset: u64,
            limit: u64,
        }

        let args = Args {
            topic,
            offset,
            limit,
        };

        let resp = self
            .execute(
                "read_batch_data",
                serde_json::to_string(&args).unwrap(),
                None,
            )
            .await?;

        let result = settled_body(resp)?;
        let result = serde_json::from_str::<Vec<StreamRecord>>(&result)?;

        Ok(result)
    }
}
