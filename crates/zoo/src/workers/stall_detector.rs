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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::uploads::create_upload;
    use chrono::Duration;

    #[tokio::test]
    async fn test_tick_no_stalled_uploads() {
        let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/zoo_test")
            .await
            .expect("failed to connect");
        let hub = Arc::new(SseHub::new());

        let detector = StallDetector::new(
            pool.clone(),
            hub,
            Duration::seconds(90).to_std().unwrap(),
        );

        let result = detector.tick().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tick_marks_stalled_upload() {
        let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/zoo_test")
            .await
            .expect("failed to connect");
        let hub = Arc::new(SseHub::new());

        let user_id = Uuid::new_v4();
        let device_id = Uuid::new_v4();

        sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) ON CONFLICT DO NOTHING")
            .bind(user_id)
            .bind(format!("stall_test_{}@test.com", Uuid::new_v4()))
            .bind("hash")
            .bind("key")
            .bind("nonce")
            .bind("salt")
            .bind(67108864i32)
            .bind(2i32)
            .bind("pub")
            .bind("sec")
            .bind("snonce")
            .bind("rec")
            .bind("rnonce")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO devices (id, user_id, name, platform, sse_token, stall_timeout_seconds) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING")
            .bind(device_id)
            .bind(user_id)
            .bind(format!("stall-device-{}", Uuid::new_v4()))
            .bind("desktop")
            .bind(Uuid::new_v4().to_string())
            .bind(90i32)
            .execute(&pool)
            .await
            .unwrap();

        let stale_time = Utc::now() - Duration::hours(2);
        let upload_id = create_upload(
            &pool,
            user_id,
            device_id,
            "stallhash",
            2000,
            Some("application/octet-stream"),
            5242880,
            1,
            Utc::now() + Duration::hours(1),
            "stall-key",
        )
        .await
        .expect("failed to create upload");

        sqlx::query("UPDATE uploads SET last_heartbeat_at = $1 WHERE id = $2")
            .bind(stale_time)
            .bind(upload_id)
            .execute(&pool)
            .await
            .unwrap();

        let detector = StallDetector::new(
            pool.clone(),
            hub,
            Duration::seconds(90).to_std().unwrap(),
        );

        detector.tick().await.expect("tick failed");

        let upload = crate::db::uploads::get_upload(&pool, upload_id)
            .await
            .expect("failed to get upload")
            .expect("upload not found");

        assert_eq!(upload.status, "stalled");
        assert!(upload.stalled_at.is_some());
    }
}
