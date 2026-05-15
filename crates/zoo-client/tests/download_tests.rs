use zoo_client::download;

#[tokio::test]
async fn test_download_file_invalid_url() {
    let client = reqwest::Client::new();
    let result = download::download_file(
        "http://invalid-url-12345",
        "token",
        1,
        &client,
    ).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_thumbnail_invalid_url() {
    let client = reqwest::Client::new();
    let result = download::get_thumbnail(
        "http://invalid-url-12345",
        "token",
        1,
        &client,
    ).await;
    assert!(result.is_err());
}
