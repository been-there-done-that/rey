use crate::error::ZooError;

pub async fn s3_put_part(url: &str, bytes: &[u8]) -> Result<String, ZooError> {
    let client = reqwest::Client::new();
    let resp = client
        .put(url)
        .body(bytes.to_vec())
        .send()
        .await
        .map_err(ZooError::HttpError)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ZooError::S3Error(format!(
            "S3 upload part failed: {} - {}",
            status, body
        )));
    }

    let etag = resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string())
        .ok_or_else(|| ZooError::S3Error("missing ETag header".to_string()))?;

    Ok(etag)
}

pub async fn s3_complete(complete_url: &str, etags: &[String]) -> Result<(), ZooError> {
    let xml_parts: String = etags
        .iter()
        .enumerate()
        .map(|(i, etag)| {
            format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                i + 1,
                etag
            )
        })
        .collect();

    let xml = format!(
        "<CompleteMultipartUpload>{}</CompleteMultipartUpload>",
        xml_parts
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(complete_url)
        .header("Content-Type", "application/xml")
        .body(xml)
        .send()
        .await
        .map_err(ZooError::HttpError)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ZooError::S3Error(format!(
            "S3 complete multipart failed: {} - {}",
            status, body
        )));
    }

    Ok(())
}
