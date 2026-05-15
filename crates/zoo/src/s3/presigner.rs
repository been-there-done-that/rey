use crate::error::ZooError;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use std::time::Duration;

pub async fn presign_part_upload(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: i32,
    ttl: Duration,
) -> Result<String, ZooError> {
    let presign_config =
        PresigningConfig::expires_in(ttl).map_err(|e| ZooError::S3(e.to_string()))?;

    let presigned = client
        .upload_part()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .part_number(part_number)
        .presigned(presign_config)
        .await
        .map_err(|e| ZooError::S3(e.to_string()))?;

    Ok(presigned.uri().to_string())
}

pub fn build_complete_url(_client: &Client, bucket: &str, key: &str, upload_id: &str) -> String {
    format!(
        "https://{}.s3.amazonaws.com/{}?uploadId={}",
        bucket, key, upload_id,
    )
}

pub async fn presign_download(
    client: &Client,
    bucket: &str,
    key: &str,
    ttl: Duration,
) -> Result<String, ZooError> {
    let presign_config =
        PresigningConfig::expires_in(ttl).map_err(|e| ZooError::S3(e.to_string()))?;

    let presigned = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(presign_config)
        .await
        .map_err(|e| ZooError::S3(e.to_string()))?;

    Ok(presigned.uri().to_string())
}
