use crate::proxy::{S3Credentials, TopicInfo, WeilStreamPublisherClient};
use serde::Serialize;
use weil_wallet::{contract::ContractId, wallet::Wallet};
use weilstream_core::PublishReceipt;

pub const MAX_PAYLOAD_SIZE: u32 = 10240; // 10Kb

pub struct WeilStreamProducer {
    weil_client: WeilStreamPublisherClient,
}

impl WeilStreamProducer {
    pub fn new(applet_id: ContractId, wallet: Wallet) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamProducer {
            weil_client: WeilStreamPublisherClient::new(applet_id, wallet)?,
        })
    }

    pub async fn with_apikey(
        applet_id: ContractId,
        api_key: String,
        creds: Option<S3Credentials>,
    ) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamProducer {
            weil_client: WeilStreamPublisherClient::with_apikey(applet_id, api_key, creds).await?,
        })
    }

    pub async fn create_topic(
        &self,
        topic: String,
        description: String,
        retention_in_bytes: u64,
    ) -> Result<(), anyhow::Error> {
        self.weil_client
            .create_topic(topic, description, retention_in_bytes)
            .await
    }

    /// Publish one event and wait for the chain to execute it.
    ///
    /// Returns the receipt the chain assigned -- sequence, txn id, block
    /// info. Callers that want to fire and forget without paying for the
    /// consensus round trip should use [`Self::publish_async`] instead.
    pub async fn publish<T: Serialize>(
        &self,
        topic: &str,
        payload: T,
    ) -> Result<PublishReceipt, anyhow::Error> {
        let serialized_payload = serde_json::to_string(&payload)?;

        if serialized_payload.len() >= MAX_PAYLOAD_SIZE as usize {
            return Err(anyhow::Error::msg("payload too large"));
        }

        self.weil_client.publish(topic, serialized_payload).await
    }

    /// Publish without waiting for the chain to execute the transaction.
    ///
    /// Returns as soon as the Sentinel accepts it. There is **no receipt**: nothing has
    /// executed yet, so there is no assigned sequence, no transaction id, and no
    /// confirmation that the publish succeeded — only that it was taken. A call that is
    /// accepted here can still fail during execution, and the caller will not hear about
    /// it.
    ///
    /// Use this to hand events over as fast as possible. Use [`Self::publish`] with
    /// [`Mode::O_DIRECT`] whenever the sequence matters, or when "it worked" has to mean
    /// more than "it was accepted".
    pub async fn publish_async<T: Serialize>(
        &self,
        topic: &str,
        payload: T,
    ) -> Result<(), anyhow::Error> {
        let serialized_payload = serde_json::to_string(&payload)?;

        if serialized_payload.len() >= MAX_PAYLOAD_SIZE as usize {
            return Err(anyhow::Error::msg("payload too large"));
        }

        self.weil_client
            .publish_async(topic, serialized_payload)
            .await
    }

    /// A topic's metadata: end offset, retention mark, chain head.
    pub async fn topic_info(&self, topic: &str) -> Result<TopicInfo, anyhow::Error> {
        self.weil_client.topic_info(topic.to_string()).await
    }

    /// Drop records below `up_to_seq`, returning the number removed.
    ///
    /// Retention, not correction: `next_seq` does not move, so a trimmed topic never
    /// reissues a sequence. Bounded per call by the applet, so trimming a long way back
    /// takes repeated calls.
    pub async fn trim(&self, topic: &str, up_to_seq: u64) -> Result<u64, anyhow::Error> {
        self.weil_client.trim(topic.to_string(), up_to_seq).await
    }
}
