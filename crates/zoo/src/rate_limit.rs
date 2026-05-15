use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RateLimitRule {
    pub max_requests: u32,
    pub window: Duration,
}

#[derive(Debug)]
struct RateLimitState {
    requests: Vec<Instant>,
}

pub struct RateLimiter {
    rules: DashMap<String, RateLimitRule>,
    state: Arc<Mutex<DashMap<String, RateLimitState>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            rules: DashMap::new(),
            state: Arc::new(Mutex::new(DashMap::new())),
        }
    }

    pub fn add_rule(&self, key: String, rule: RateLimitRule) {
        self.rules.insert(key, rule);
    }

    pub async fn check(&self, key: &str) -> Result<(), Duration> {
        let rule = match self.rules.get(key) {
            Some(r) => r.clone(),
            None => return Ok(()),
        };

        let now = Instant::now();
        let cutoff = now - rule.window;

        let state_map = self.state.lock().await;
        let mut state = state_map
            .entry(key.to_string())
            .or_insert(RateLimitState {
                requests: Vec::new(),
            });

        state.requests.retain(|t| *t > cutoff);

        if state.requests.len() >= rule.max_requests as usize {
            let oldest = state.requests.first().unwrap();
            let retry_after = rule.window - now.duration_since(*oldest);
            return Err(retry_after);
        }

        state.requests.push(now);
        Ok(())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new();
        limiter.add_rule(
            "test".to_string(),
            RateLimitRule {
                max_requests: 3,
                window: Duration::from_secs(60),
            },
        );

        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_rejects_over_limit() {
        let limiter = RateLimiter::new();
        limiter.add_rule(
            "test".to_string(),
            RateLimitRule {
                max_requests: 2,
                window: Duration::from_secs(60),
            },
        );

        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());
        let result = limiter.check("test").await;
        assert!(result.is_err());
    }
}
