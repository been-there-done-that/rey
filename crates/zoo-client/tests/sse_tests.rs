use zoo_client::sse::SseClient;

#[test]
fn test_sse_client_new() {
    let client = SseClient::new("http://localhost:3000".to_string(), "token".to_string());
    // Just verify it constructs without panicking
    drop(client);
}
