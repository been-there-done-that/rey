use zoo_client::upload;

#[tokio::test]
async fn test_s3_put_part_invalid_url() {
    let result = upload::s3_put_part("http://invalid-url-12345/part", &[0u8; 10]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        zoo_client::ZooError::HttpError(_) | zoo_client::ZooError::S3Error(_) => {}
        _ => panic!("expected HTTP or S3 error"),
    }
}

#[tokio::test]
async fn test_s3_complete_invalid_url() {
    let result =
        upload::s3_complete("http://invalid-url-12345/complete", &["etag1".to_string()]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        zoo_client::ZooError::HttpError(_) | zoo_client::ZooError::S3Error(_) => {}
        _ => panic!("expected HTTP or S3 error"),
    }
}
