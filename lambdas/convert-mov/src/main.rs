use aws_config::BehaviorVersion;
use aws_sdk_mediaconvert::types::{
    AvcIntraClass, ContainerSettings, FileGroupSettings, H264Settings, Input, JobSettings, Output,
    OutputGroup, OutputGroupSettings, VideoCodecSettings, VideoDescription,
};
use aws_sdk_mediaconvert::Client;
use lambda_runtime::{run, service_fn, Error, LambdaEvent, tracing};
use serde_json::{json, Value};

async fn function_handler(event: LambdaEvent<Value>) -> Result<(), Error> {
    // Extract input and output paths from the event
    // NOTE: We are using input transformation so we are limiting the payload
    let input_bucket = event.payload["input_bucket"]
        .as_str()
        .ok_or("Input bucket not provided")?;
    let input_key = event.payload["input_key"]
        .as_str()
        .ok_or("Input key not provided")?;
    // NOTE: This will most likely always be the same bucket
    let output_bucket = event.payload["output_bucket"] 
        .as_str()
        .ok_or("Output bucket not provided")?;
    let output_key = format!("converted_video/{}", &input_key);

    // Load the AWS configuration
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;

    // Create MediaConvert client
    let client = Client::new(&config);

    // Create job request
    // TODO: Handle this request result
    let job_request = create_job_request(input_bucket, input_key, output_bucket, &output_key);

    // Submit job
    println!("We are creating a MediaConvert Video processing job");
    let response = client
        .create_job()
        .role("arn:aws:iam::824852318651:role/MediaConvertS3".to_string())
        .settings(job_request)
        .send()
        .await;
    // Handle the create_job
    match response {
        Ok(_) => println!("Successfully submitted the media processing job"),
        Err(e) => eprintln!("We were not able to submit the job: {:?}", e),
    }

    Ok(())
}

fn create_job_request(
    input_bucket: &str,
    input_key: &str,
    output_bucket: &str,
    output_key: &str,
) -> JobSettings {
    JobSettings::builder()
        .inputs(
            Input::builder()
                .file_input(format!("s3://{}/{}", input_bucket, input_key))
                .build(),
        )
        .output_groups(
            OutputGroup::builder()
                .output_group_settings(
                    OutputGroupSettings::builder()
                        .r#type(aws_sdk_mediaconvert::types::OutputGroupType::FileGroupSettings)
                        .file_group_settings(
                            FileGroupSettings::builder()
                                .destination(format!("s3://{}/{}", output_bucket, output_key))
                                .build(),
                        )
                        .build(),
                )
                .outputs(
                    Output::builder()
                        .container_settings(
                            ContainerSettings::builder()
                                .container(aws_sdk_mediaconvert::types::ContainerType::Mp4)
                                .build(),
                        )
                        .video_description(
                            VideoDescription::builder()
                                .codec_settings(
                                    VideoCodecSettings::builder()
                                        .codec(aws_sdk_mediaconvert::types::VideoCodec::H264)
                                        .h264_settings(
                                            H264Settings::builder()
                                                .rate_control_mode(aws_sdk_mediaconvert::types::H264RateControlMode::Cbr)
                                                .bitrate(5000000)
                                                .build(),
                                        )
                                        .build(),
                                )
                                .build(),
                        )
                        .build(),
                )
                .build(),
        )
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(function_handler)).await
}
