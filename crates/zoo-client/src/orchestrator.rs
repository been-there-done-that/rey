use crate::download;
use crate::error::ZooError;
use crate::upload;
use base64::Engine;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use types::upload::{UploadState, UploadStatus};
use uuid::Uuid;

const DEFAULT_PART_SIZE: usize = 20 * 1024 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_PART_COUNT: usize = 5;
const MAX_PRESIGN_RETRIES: u32 = 3;

#[derive(Clone)]
pub struct ZooClient {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    base_url: String,
    session_token: RwLock<Option<String>>,
    client: reqwest::Client,
}

impl ZooClient {
    pub fn new(base_url: String) -> Self {
        Self {
            inner: Arc::new(ClientInner {
                base_url,
                session_token: RwLock::new(None),
                client: reqwest::Client::builder()
                    .timeout(Duration::from_secs(300))
                    .build()
                    .unwrap(),
            }),
        }
    }

    pub fn set_session_token(&self, token: String) {
        let mut lock = futures::executor::block_on(self.inner.session_token.write());
        *lock = Some(token);
    }

    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    pub async fn session_token(&self) -> Option<String> {
        self.inner.session_token.read().await.clone()
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.inner.client
    }

    async fn get_token(&self) -> Result<String, ZooError> {
        let lock = self.inner.session_token.read().await;
        lock.clone().ok_or(ZooError::NotAuthenticated)
    }

    pub async fn upload_file(
        &self,
        source_bytes: &[u8],
        file_hash: &str,
        part_md5s: Vec<String>,
        file_size: i64,
        mime_type: &str,
        collection_id: &str,
    ) -> Result<i64, ZooError> {
        let token = self.get_token().await?;
        let part_count = part_md5s.len() as u16;
        let part_size = DEFAULT_PART_SIZE as i32;

        let upload_id = self
            .create_upload(
                &token,
                file_hash,
                file_size,
                mime_type,
                part_size,
                part_count,
                collection_id,
            )
            .await?;

        info!("created upload {}", upload_id);

        let upload_id_str = upload_id.to_string();

        self.patch_upload(&token, &upload_id_str, "encrypting")
            .await?;

        let presign_resp = self
            .presign_urls(&token, &upload_id_str, &part_md5s)
            .await?;

        self.patch_upload(&token, &upload_id_str, "uploading")
            .await?;

        let etags = self
            .upload_parts_with_heartbeat(
                &token,
                &upload_id_str,
                source_bytes,
                &presign_resp.urls,
                part_size as usize,
            )
            .await?;

        upload::s3_complete(&presign_resp.complete_url, &etags).await?;

        self.patch_upload(&token, &upload_id_str, "s3_completed")
            .await?;

        let file_id = self.register_upload(&token, &upload_id_str).await?;

        info!("upload {} completed, file_id={}", upload_id, file_id);
        Ok(file_id)
    }

    pub async fn resume_upload(
        &self,
        upload_id: Uuid,
        source_bytes: &[u8],
    ) -> Result<i64, ZooError> {
        let token = self.get_token().await?;

        self.patch_upload(&token, &upload_id.to_string(), "resuming")
            .await?;

        let state = self.get_upload(&token, &upload_id.to_string()).await?;

        if state.status == UploadStatus::Failed {
            return Err(ZooError::UploadAborted);
        }

        let presign_resp = self.presign_refresh(&token, &upload_id.to_string()).await?;

        let missing_parts = self.find_missing_parts(&state, source_bytes.len())?;

        let etags = self
            .upload_missing_parts(
                &token,
                &upload_id,
                source_bytes,
                &presign_resp.urls,
                &missing_parts,
                state.part_size as usize,
            )
            .await?;

        upload::s3_complete(&presign_resp.complete_url, &etags).await?;

        self.patch_upload(&token, &upload_id.to_string(), "s3_completed")
            .await?;

        let file_id = self.register_upload(&token, &upload_id.to_string()).await?;

        info!("resume upload {} completed, file_id={}", upload_id, file_id);
        Ok(file_id)
    }

    pub async fn pending_uploads(&self) -> Result<Vec<UploadState>, ZooError> {
        let token = self.get_token().await?;
        let url = format!("{}/api/uploads?status=pending", self.inner.base_url);
        let resp = self
            .inner
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if resp.status() == 404 {
            return Ok(vec![]);
        }

        let uploads: Vec<UploadState> = resp.json().await.map_err(ZooError::HttpError)?;
        Ok(uploads)
    }

    pub async fn cancel_upload(&self, upload_id: Uuid) -> Result<(), ZooError> {
        let token = self.get_token().await?;
        let url = format!("{}/api/uploads/{}", self.inner.base_url, upload_id);
        let resp = self
            .inner
            .client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::StateError(format!(
                "cancel upload failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    pub async fn download_file(&self, file_id: i64) -> Result<Vec<u8>, ZooError> {
        let token = self.get_token().await?;
        download::download_file(&self.inner.base_url, &token, file_id, &self.inner.client).await
    }

    pub async fn get_thumbnail(&self, file_id: i64) -> Result<Vec<u8>, ZooError> {
        let token = self.get_token().await?;
        download::get_thumbnail(&self.inner.base_url, &token, file_id, &self.inner.client).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_upload(
        &self,
        token: &str,
        file_hash: &str,
        file_size: i64,
        mime_type: &str,
        part_size: i32,
        part_count: u16,
        collection_id: &str,
    ) -> Result<Uuid, ZooError> {
        let url = format!("{}/api/uploads", self.inner.base_url);
        let body = serde_json::json!({
            "file_hash": file_hash,
            "file_size": file_size,
            "mime_type": mime_type,
            "part_size": part_size,
            "part_count": part_count,
            "collection_id": collection_id,
        });

        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if resp.status() == 409 {
            let body: serde_json::Value = resp.json().await.map_err(ZooError::HttpError)?;
            let existing_id = body["upload_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| {
                    ZooError::Conflict("duplicate upload, no ID returned".to_string())
                })?;
            return Err(ZooError::Conflict(format!(
                "duplicate upload: {}",
                existing_id
            )));
        }

        let result: serde_json::Value = resp.json().await.map_err(ZooError::HttpError)?;
        let upload_id = result["upload_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| ZooError::ParseError("missing upload_id in response".to_string()))?;

        Ok(upload_id)
    }

    async fn patch_upload(
        &self,
        token: &str,
        upload_id: &str,
        status: &str,
    ) -> Result<(), ZooError> {
        let url = format!("{}/api/uploads/{}", self.inner.base_url, upload_id);
        let body = serde_json::json!({ "status": status });

        let resp = self
            .inner
            .client
            .patch(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::StateError(format!(
                "patch upload failed: {} - {}",
                status_code, body
            )));
        }

        Ok(())
    }

    async fn presign_urls(
        &self,
        token: &str,
        upload_id: &str,
        part_md5s: &[String],
    ) -> Result<PresignResponse, ZooError> {
        let url = format!("{}/api/uploads/{}/presign", self.inner.base_url, upload_id);
        let body = serde_json::json!({ "part_md5s": part_md5s });

        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::ParseError(format!(
                "presign failed: {} - {}",
                status_code, body
            )));
        }

        let result: PresignResponse = resp.json().await.map_err(ZooError::HttpError)?;
        Ok(result)
    }

    async fn presign_refresh(
        &self,
        token: &str,
        upload_id: &str,
    ) -> Result<PresignResponse, ZooError> {
        let url = format!(
            "{}/api/uploads/{}/presign-refresh",
            self.inner.base_url, upload_id
        );

        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::ParseError(format!(
                "presign-refresh failed: {} - {}",
                status_code, body
            )));
        }

        let result: PresignResponse = resp.json().await.map_err(ZooError::HttpError)?;
        Ok(result)
    }

    async fn get_upload(&self, token: &str, upload_id: &str) -> Result<UploadState, ZooError> {
        let url = format!("{}/api/uploads/{}", self.inner.base_url, upload_id);
        let resp = self
            .inner
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::ParseError(format!(
                "get upload failed: {} - {}",
                status_code, body
            )));
        }

        let state: UploadState = resp.json().await.map_err(ZooError::HttpError)?;
        Ok(state)
    }

    async fn register_upload(&self, token: &str, upload_id: &str) -> Result<i64, ZooError> {
        let url = format!("{}/api/uploads/{}/register", self.inner.base_url, upload_id);

        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(ZooError::HttpError)?;

        if !resp.status().is_success() {
            let status_code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ZooError::ParseError(format!(
                "register upload failed: {} - {}",
                status_code, body
            )));
        }

        let result: serde_json::Value = resp.json().await.map_err(ZooError::HttpError)?;
        let file_id = result["file_id"].as_i64().ok_or_else(|| {
            ZooError::ParseError("missing file_id in register response".to_string())
        })?;

        Ok(file_id)
    }

    async fn upload_parts_with_heartbeat(
        &self,
        token: &str,
        upload_id: &str,
        source_bytes: &[u8],
        urls: &[String],
        part_size: usize,
    ) -> Result<Vec<String>, ZooError> {
        let mut etags = Vec::with_capacity(urls.len());
        let mut last_heartbeat = std::time::Instant::now();
        let mut parts_since_heartbeat = 0;

        for (i, url) in urls.iter().enumerate() {
            let start = i * part_size;
            let end = std::cmp::min(start + part_size, source_bytes.len());
            let part_bytes = &source_bytes[start..end];

            let etag = self
                .upload_part_with_retry(url, part_bytes, MAX_PRESIGN_RETRIES)
                .await?;

            etags.push(etag);
            parts_since_heartbeat += 1;

            if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL
                || parts_since_heartbeat >= HEARTBEAT_PART_COUNT
            {
                self.patch_upload(token, upload_id, "uploading").await?;
                last_heartbeat = std::time::Instant::now();
                parts_since_heartbeat = 0;
                debug!("heartbeat sent for upload {}", upload_id);
            }
        }

        Ok(etags)
    }

    async fn upload_part_with_retry(
        &self,
        url: &str,
        bytes: &[u8],
        max_retries: u32,
    ) -> Result<String, ZooError> {
        let mut retries = 0;
        loop {
            match upload::s3_put_part(url, bytes).await {
                Ok(etag) => return Ok(etag),
                Err(ZooError::S3Error(msg)) if msg.contains("403") && retries < max_retries => {
                    retries += 1;
                    warn!("S3 403 on part upload, retry {}/{}", retries, max_retries);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn find_missing_parts(
        &self,
        state: &UploadState,
        _total_size: usize,
    ) -> Result<Vec<usize>, ZooError> {
        let part_count = state.part_count as usize;
        let _part_size = state.part_size as usize;
        let mut missing = Vec::new();

        let bitmask = if state.parts_bitmask.is_empty() {
            Vec::new()
        } else {
            base64::prelude::BASE64_STANDARD
                .decode(&state.parts_bitmask)
                .unwrap_or_default()
        };

        for i in 0..part_count {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let is_uploaded = bitmask
                .get(byte_idx)
                .map(|byte| (byte >> bit_idx) & 1 == 1)
                .unwrap_or(false);

            if !is_uploaded {
                missing.push(i);
            }
        }

        Ok(missing)
    }

    async fn upload_missing_parts(
        &self,
        token: &str,
        upload_id: &Uuid,
        source_bytes: &[u8],
        urls: &[String],
        missing_indices: &[usize],
        part_size: usize,
    ) -> Result<Vec<String>, ZooError> {
        let mut etags = Vec::new();
        let mut last_heartbeat = std::time::Instant::now();
        let mut parts_since_heartbeat = 0;

        for &idx in missing_indices {
            if idx >= urls.len() {
                return Err(ZooError::ParseError(format!(
                    "missing part index {} out of range ({} urls)",
                    idx,
                    urls.len()
                )));
            }

            let start = idx * part_size;
            let end = std::cmp::min(start + part_size, source_bytes.len());
            let part_bytes = &source_bytes[start..end];

            let etag = self
                .upload_part_with_retry(&urls[idx], part_bytes, MAX_PRESIGN_RETRIES)
                .await?;

            etags.push(etag);
            parts_since_heartbeat += 1;

            if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL
                || parts_since_heartbeat >= HEARTBEAT_PART_COUNT
            {
                self.patch_upload(token, &upload_id.to_string(), "uploading")
                    .await?;
                last_heartbeat = std::time::Instant::now();
                parts_since_heartbeat = 0;
            }
        }

        Ok(etags)
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PresignResponse {
    urls: Vec<String>,
    complete_url: String,
}
