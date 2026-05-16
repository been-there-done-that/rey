mod config;

use config::ZooConfig;
use wasm_bindgen::prelude::*;
use zoo_client::ZooClient;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct ZooHandle {
    client: ZooClient,
}

#[wasm_bindgen]
impl ZooHandle {
    #[wasm_bindgen(constructor)]
    pub async fn create(config: JsValue) -> Result<ZooHandle, JsError> {
        let config: ZooConfig = serde_wasm_bindgen::from_value(config)?;
        let client = ZooClient::new(config.base_url);
        Ok(ZooHandle { client })
    }

    pub async fn upload_file(
        &self,
        encrypted_bytes: &[u8],
        file_hash: &str,
        part_md5s: JsValue,
        file_size: i64,
        mime_type: &str,
        collection_id: &str,
    ) -> Result<JsValue, JsError> {
        let part_md5s: Vec<String> = serde_wasm_bindgen::from_value(part_md5s)?;
        let file_id = self
            .client
            .upload_file(
                encrypted_bytes,
                file_hash,
                part_md5s,
                file_size,
                mime_type,
                collection_id,
            )
            .await?;
        Ok(serde_wasm_bindgen::to_value(&file_id)?)
    }

    pub async fn resume_upload(
        &self,
        upload_id: &str,
        encrypted_bytes: &[u8],
    ) -> Result<JsValue, JsError> {
        let upload_id = uuid::Uuid::parse_str(upload_id)?;
        let file_id = self
            .client
            .resume_upload(upload_id, encrypted_bytes)
            .await?;
        Ok(serde_wasm_bindgen::to_value(&file_id)?)
    }

    pub async fn pending_uploads(&self) -> Result<JsValue, JsError> {
        let uploads = self.client.pending_uploads().await?;
        Ok(serde_wasm_bindgen::to_value(&uploads)?)
    }

    pub async fn cancel_upload(&self, upload_id: &str) -> Result<(), JsError> {
        let upload_id = uuid::Uuid::parse_str(upload_id)?;
        self.client.cancel_upload(upload_id).await?;
        Ok(())
    }

    pub async fn download_file(&self, file_id: i64) -> Result<JsValue, JsError> {
        let bytes = self.client.download_file(file_id).await?;
        Ok(serde_wasm_bindgen::to_value(&bytes)?)
    }

    pub async fn get_thumbnail(&self, file_id: i64) -> Result<JsValue, JsError> {
        let bytes = self.client.get_thumbnail(file_id).await?;
        Ok(serde_wasm_bindgen::to_value(&bytes)?)
    }

    pub fn set_session_token(&self, token: &str) {
        self.client.set_session_token(token.to_string());
    }

    pub fn close(&self) {
        // Cleanup if needed
    }
}
