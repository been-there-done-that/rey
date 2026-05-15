use crate::error::ZooError;

pub async fn download_file(
    base_url: &str,
    session_token: &str,
    file_id: i64,
    client: &reqwest::Client,
) -> Result<Vec<u8>, ZooError> {
    let url = format!("{}/api/files/{}/download", base_url, file_id);
    let resp = client
        .get(&url)
        .bearer_auth(session_token)
        .send()
        .await
        .map_err(ZooError::HttpError)?;

    if resp.status().is_redirection() {
        let location = resp
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ZooError::ParseError("missing Location header".to_string()))?;

        let body = reqwest::get(location)
            .await
            .map_err(ZooError::HttpError)?
            .bytes()
            .await
            .map_err(ZooError::HttpError)?;

        return Ok(body.to_vec());
    }

    let bytes = resp.bytes().await.map_err(ZooError::HttpError)?;
    Ok(bytes.to_vec())
}

pub async fn get_thumbnail(
    base_url: &str,
    session_token: &str,
    file_id: i64,
    client: &reqwest::Client,
) -> Result<Vec<u8>, ZooError> {
    let url = format!("{}/api/files/{}/thumbnail", base_url, file_id);
    let resp = client
        .get(&url)
        .bearer_auth(session_token)
        .send()
        .await
        .map_err(ZooError::HttpError)?;

    let bytes = resp.bytes().await.map_err(ZooError::HttpError)?;
    Ok(bytes.to_vec())
}
