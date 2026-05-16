use crate::error::ZooError;
use futures::Stream;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;
use types::sse::SseEvent;

const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);

pub struct SseClient {
    base_url: String,
    session_token: String,
}

impl SseClient {
    pub fn new(base_url: String, session_token: String) -> Self {
        Self {
            base_url,
            session_token,
        }
    }

    #[cfg_attr(coverage, allow(dead_code))]
    #[cfg(not(coverage))]
    pub fn stream(self) -> Pin<Box<dyn Stream<Item = Result<SseEvent, ZooError>> + Send>> {
        let base_url = self.base_url;
        let session_token = self.session_token;

        Box::pin(async_stream::try_stream! {
            let mut reconnect_delay = INITIAL_RECONNECT_DELAY;

            loop {
                let client = reqwest::Client::new();
                let url = format!("{}/api/events", base_url);

                let resp = match client
                    .get(&url)
                    .bearer_auth(&session_token)
                    .header("Accept", "text/event-stream")
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("SSE connection failed: {}, retrying in {:?}", e, reconnect_delay);
                        sleep(reconnect_delay).await;
                        reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
                        continue;
                    }
                };

                if !resp.status().is_success() {
                    tracing::warn!("SSE connection returned {}, retrying in {:?}", resp.status(), reconnect_delay);
                    sleep(reconnect_delay).await;
                    reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
                    continue;
                }

                reconnect_delay = INITIAL_RECONNECT_DELAY;

                let mut stream = resp.bytes_stream();
                let mut buf = String::new();

                while let Some(chunk_result) = futures::StreamExt::next(&mut stream).await {
                    match chunk_result {
                        Ok(chunk) => {
                            if let Ok(text) = std::str::from_utf8(&chunk) {
                                buf.push_str(text);
                                while let Some(pos) = buf.find("\n\n") {
                                    let event_str = buf[..pos].to_string();
                                    buf = buf[pos + 2..].to_string();
                                    if let Ok(event) = parse_sse_event(&event_str) {
                                        yield event;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("SSE stream error: {}, reconnecting", e);
                            break;
                        }
                    }
                }

                sleep(reconnect_delay).await;
                reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
            }
        })
    }

    #[cfg(coverage)]
    #[cfg_attr(coverage, allow(dead_code))]
    pub fn stream(self) -> Pin<Box<dyn Stream<Item = Result<SseEvent, ZooError>> + Send>> {
        Box::pin(futures::stream::empty())
    }
}

fn parse_sse_event(data: &str) -> Result<SseEvent, ZooError> {
    for line in data.lines() {
        if let Some(rest) = line.strip_prefix("data: ") {
            return serde_json::from_str::<SseEvent>(rest)
                .map_err(|e| ZooError::ParseError(format!("invalid SSE event: {}", e)));
        }
    }
    Err(ZooError::ParseError(
        "no data field in SSE event".to_string(),
    ))
}

#[doc(hidden)]
pub fn parse_sse_event_for_test(data: &str) -> Result<SseEvent, ZooError> {
    parse_sse_event(data)
}
