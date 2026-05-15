use crate::db::uploads::{list_stalled_uploads, mark_stalled};
use crate::sse::hub::SseHub;
use base64::Engine;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::info;
use types::sse::SseEvent;

pub struct StallDetector {
    pool: PgPool,
    hub: Arc<SseHub>,
    stall_timeout: Duration,
}

impl StallDetector {
    pub fn new(pool: PgPool, hub: Arc<SseHub>, stall_timeout: Duration) -> Self {
        Self {
            pool,
            hub,
            stall_timeout,
        }
    }

    pub async fn run(self) {
        let mut interval = interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                tracing::error!("stall detector error: {e}");
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let threshold = Utc::now() - chrono::Duration::from_std(self.stall_timeout)?;
        let stalled = list_stalled_uploads(&self.pool, threshold).await?;

        for upload in stalled {
            mark_stalled(&self.pool, upload.id).await?;
            info!("upload {} marked as stalled", upload.id);

            let event = SseEvent::UploadStalled {
                upload_id: upload.id.to_string(),
                parts_bitmask: upload
                    .parts_bitmask
                    .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
                    .unwrap_or_default(),
                part_count: upload.part_count as u16,
                device_name: upload.device_id.to_string(),
                stalled_at: Utc::now().timestamp_millis(),
            };
            self.hub.broadcast(&upload.user_id.to_string(), event);
        }

        Ok(())
    }
}
