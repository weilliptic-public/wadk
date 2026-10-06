use std::str::FromStr;
use weil_wallet::contract::ContractId;
use weilstream_sdk::{
    consumer::WeilStreamConsumer, producer::WeilStreamProducer, proxy::S3Credentials,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let publisher_api_key = "weil_abcd";
    let consumer_api_key = "weil_abcd";

    let creds = S3Credentials {
        access_key_id: "access_key".to_string(),
        secret_access_key: "secret_key".to_string(),
        region: "region".to_string(),
        bucket_name: "bucket".to_string(),
    };

    let consumer_applet_id =
        ContractId::from_str("<consumer_applet_id>")?;
    let mut consumer = WeilStreamConsumer::with_api_key(
        consumer_applet_id,
        10,
        consumer_api_key.to_string(),
        Some(creds.clone()),
    )
    .await?;

    let producer_applet_id =
        ContractId::from_str("<producer_applet_id>")?;
    let producer = WeilStreamProducer::with_apikey(
        producer_applet_id,
        publisher_api_key.to_string(),
        Some(creds),
    )
    .await?;

    consumer.subscribe(&["<topic>".to_string()]);
    let r = consumer.poll(None).await?;
    println!("Records: {:?}", r);

    println!("-----------------------------------------------");

    for i in 0..100 {
        producer
            .publish_async("<topic>", format!("New record {}", i))
            .await?;

        let r = consumer.poll(None).await?;
        println!("Records: {:?}", r);

        println!("-----------------------------------------------");
    }

    Ok(())
}
