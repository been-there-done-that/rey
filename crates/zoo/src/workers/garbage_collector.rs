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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::uploads::create_upload;
    use chrono::Duration;

    #[tokio::test]
    async fn test_tick_no_expired_uploads() {
        let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/zoo_test")
            .await
            .expect("failed to connect");
        let s3_client = Client::new();
        let hub = Arc::new(SseHub::new());

        let gc = GarbageCollector::new(
            pool.clone(),
            s3_client,
            hub,
            "test-bucket".to_string(),
            Duration::seconds(300).to_std().unwrap(),
        );

        let result = gc.tick().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tick_marks_expired_upload_as_failed() {
        let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/zoo_test")
            .await
            .expect("failed to connect");
        let s3_client = Client::new();
        let hub = Arc::new(SseHub::new());

        let user_id = Uuid::new_v4();
        let device_id = Uuid::new_v4();

        sqlx::query("INSERT INTO users (id, email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt, mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce, encrypted_recovery_key, recovery_key_nonce) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) ON CONFLICT DO NOTHING")
            .bind(user_id)
            .bind(format!("gc_test_{}@test.com", Uuid::new_v4()))
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
            .bind(format!("gc-device-{}", Uuid::new_v4()))
            .bind("desktop")
            .bind(Uuid::new_v4().to_string())
            .bind(90i32)
            .execute(&pool)
            .await
            .unwrap();

        let expired_time = Utc::now() - Duration::hours(2);
        let upload_id = create_upload(
            &pool,
            user_id,
            device_id,
            "testhash",
            1000,
            Some("application/octet-stream"),
            5242880,
            1,
            expired_time,
            "test-key",
        )
        .await
        .expect("failed to create upload");

        let gc = GarbageCollector::new(
            pool.clone(),
            s3_client,
            hub,
            "test-bucket".to_string(),
            Duration::seconds(300).to_std().unwrap(),
        );

        gc.tick().await.expect("tick failed");

        let upload = crate::db::uploads::get_upload(&pool, upload_id)
            .await
            .expect("failed to get upload")
            .expect("upload not found");

        assert_eq!(upload.status, "failed");
    }
}
