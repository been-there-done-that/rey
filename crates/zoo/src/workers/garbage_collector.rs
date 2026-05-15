use crate::db::uploads::{list_expired_uploads, patch_upload_status};
use crate::s3::client::abort_multipart_upload;
use crate::sse::hub::SseHub;
use aws_sdk_s3::Client;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::info;
use types::sse::SseEvent;

pub struct GarbageCollector {
    pool: PgPool,
    s3_client: Client,
    hub: Arc<SseHub>,
    bucket: String,
    interval: Duration,
}

impl GarbageCollector {
    pub fn new(
        pool: PgPool,
        s3_client: Client,
        hub: Arc<SseHub>,
        bucket: String,
        interval: Duration,
    ) -> Self {
        Self {
            pool,
            s3_client,
            hub,
            bucket,
            interval,
        }
    }

    pub async fn run(self) {
        let mut interval = interval(self.interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                tracing::error!("garbage collector error: {e}");
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let expired = list_expired_uploads(&self.pool, Utc::now()).await?;

        for upload in expired {
            if let Some(s3_upload_id) = &upload.upload_id_s3 {
                if let Some(object_key) = &upload.object_key {
                    let _ = abort_multipart_upload(
                        &self.s3_client,
                        &self.bucket,
                        object_key,
                        s3_upload_id,
                    )
                    .await;
                }
            }

            patch_upload_status(&self.pool, upload.id, "failed").await?;
            info!("upload {} marked as failed by GC", upload.id);

            let event = SseEvent::UploadFailed {
                upload_id: upload.id.to_string(),
                reason: "upload expired".to_string(),
                device_name: upload.device_id.to_string(),
            };
            self.hub.broadcast(&upload.user_id.to_string(), event);
        }

        Ok(())
    }
}
