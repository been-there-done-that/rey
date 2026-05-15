use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Region, Client};
use crate::config::ZooConfig;
use crate::error::ZooError;

pub async fn create_client(config: &ZooConfig) -> Result<Client, ZooError> {
    let mut sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.s3_region.clone()))
        .load()
        .await;

    let mut config_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

    if let Some(endpoint) = &config.s3_endpoint {
        config_builder = config_builder.endpoint_url(endpoint);
        config_builder = config_builder.force_path_style(true);
    }

    let client = Client::from_conf(config_builder.build());
    Ok(client)
}

pub async fn head_object_size(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<i64, ZooError> {
    let resp = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| ZooError::S3(e.to_string()))?;

    let size = resp.content_length().unwrap_or(0);
    Ok(size as i64)
}

pub async fn abort_multipart_upload(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> Result<(), ZooError> {
    client
        .abort_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .send()
        .await
        .map_err(|e| ZooError::S3(e.to_string()))?;
    Ok(())
}

pub async fn delete_object(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<(), ZooError> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| ZooError::S3(e.to_string()))?;
    Ok(())
}
