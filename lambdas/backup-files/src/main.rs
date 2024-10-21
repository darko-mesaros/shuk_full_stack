use aws_config::{meta::region::RegionProviderChain, Region};
use aws_sdk_s3::Client;
use aws_lambda_events::event::sns::SnsEvent;
use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use serde::{Deserialize, Serialize};


// There is a lot of Struct scaffolding for all these events - thats Rust for you 🦀
// --- SNS START ---
#[derive(Deserialize, Serialize)]
struct SNSEvent {
    records: Vec<SNSRecord>,
}

#[derive(Deserialize, Serialize)]
struct SNSRecord {
    sns: SNSMessage,
}

#[derive(Deserialize, Serialize)]
struct SNSMessage {
    message: String,
}
// --- SNS END ---

// --- EVENT BRIDGE START ---
#[derive(Deserialize, Serialize)]
struct EventBridgeEvent {
    source: String,
    #[serde(rename = "detail-type")]
    detail_type: String,
    time: String,
    detail: S3EventDetail,
}
// --- EVENT BRIDGE END ---

// --- S3 EVENT START ---
#[derive(Deserialize, Serialize)]
struct S3EventDetail {
    version: String,
    bucket: S3Bucket,
    object: S3Object,
    #[serde(rename = "request-id")]
    request_id: String,
    requester: String,
    #[serde(rename = "source-ip-address")]
    source_ip_address: String,
    reason: String,
}

#[derive(Deserialize, Serialize)]
struct S3Bucket {
    name: String,
}

#[derive(Deserialize, Serialize)]
struct S3Object {
    key: String,
    size: u64,
    etag: String,
    #[serde(rename = "version-id")]
    version_id: Option<String>,
    sequencer: String,
}
// --- S3 EVENT END ---

async fn function_handler(event: LambdaEvent<SnsEvent>) -> Result<(), Error> {
    // Extract some useful information from the request
    let sns_event = event.payload;

    // Configure the AWS SDK
    let region_provider = RegionProviderChain::default_provider().or_else(Region::new("us-west-2"));
    let config = aws_config::defaults(
        aws_config::BehaviorVersion::latest()
        ).region(region_provider).load().await;

    let s3_client = Client::new(&config);

    for record in sns_event.records {
        // Parse the event bridge event from the SNS message
        let eventbridge_event: EventBridgeEvent = serde_json::from_str(&record.sns.message)?;

        // Process the S3 event details
        let s3_detail = eventbridge_event.detail;

        let source_bucket = s3_detail.bucket.name;
        let source_key = s3_detail.object.key;
        let destination_bucket = "aws-darko-videos-backups".to_string();
        let destination_key = format!("backed_up/{}", source_key);

        println!("Backing Up Object s3://{}/{} to s3://{}/{}",&source_bucket, &source_key, &destination_bucket, &destination_key);
        match copy_object(&s3_client, &source_bucket, &source_key, &destination_bucket, &destination_key).await {
            Ok(_) => {
                match tag_object(&s3_client, &source_bucket, &source_key).await {
                    Ok(_) => println!("Object succesfully Tagged"),
                    Err(e) => eprintln!("Your object was uploaded, but there was an issue tagging the original: {}",e),
                }
                println!("Backup worked");
            }
            Err(e) => eprintln!("It's clogged: {:?}", e),
        }
    }

    Ok(())
}

async fn copy_object(client: &Client, source_bucket: &str, source_key: &str, dest_bucket: &str, dest_key: &str) -> Result<(), aws_sdk_s3::Error> {
    client.copy_object()
        .copy_source(format!("{}/{}", source_bucket, source_key))
        .bucket(dest_bucket)
        .key(dest_key)
        .send()
        .await?;

        Ok(())
}

async fn tag_object(client: &Client, source_bucket: &str, source_key: &str) -> Result<(), aws_sdk_s3::Error> {
    let tag = aws_sdk_s3::types::Tag::builder()
        .key("backed_up")
        .value("TRUE")
        .build()?;

    let tags = aws_sdk_s3::types::Tagging::builder()
        .tag_set(tag)
        .build()?;

    client.put_object_tagging()
        .bucket(source_bucket)
        .key(source_key)
        .tagging(tags)
        .send()
        .await?;

        Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    run(service_fn(function_handler)).await
}
