use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use types::sse::SseEvent;

const BUFFER_SIZE: usize = 256;

pub struct SseHub {
    senders: Arc<DashMap<String, broadcast::Sender<SseEvent>>>,
}

impl SseHub {
    pub fn new() -> Self {
        Self {
            senders: Arc::new(DashMap::new()),
        }
    }

    pub fn subscribe(&self, user_id: &str) -> broadcast::Receiver<SseEvent> {
        if let Some(entry) = self.senders.get(user_id) {
            entry.value().subscribe()
        } else {
            let (tx, rx) = broadcast::channel(BUFFER_SIZE);
            self.senders.insert(user_id.to_string(), tx);
            rx
        }
    }

    pub fn broadcast(&self, user_id: &str, event: SseEvent) {
        if let Some(entry) = self.senders.get(user_id) {
            let _ = entry.value().send(event);
        }
    }

    pub fn cleanup_if_empty(&self, user_id: &str) {
        if let Some(entry) = self.senders.get(user_id) {
            if entry.value().receiver_count() == 0 {
                self.senders.remove(user_id);
            }
        }
    }

    pub fn sender_count(&self, user_id: &str) -> usize {
        self.senders
            .get(user_id)
            .map(|e| e.value().receiver_count())
            .unwrap_or(0)
    }
}

impl Default for SseHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_and_broadcast() {
        let hub = SseHub::new();
        let mut rx = hub.subscribe("user-1");
        let event = SseEvent::Heartbeat { timestamp: 1700000000000 };
        hub.broadcast("user-1", event.clone());
        let received = rx.try_recv().unwrap();
        match received {
            SseEvent::Heartbeat { timestamp } => assert_eq!(timestamp, 1700000000000),
            _ => panic!("wrong event type"),
        }
    }

    #[test]
    fn test_cleanup_if_empty() {
        let hub = SseHub::new();
        let _rx = hub.subscribe("user-1");
        drop(_rx);
        hub.cleanup_if_empty("user-1");
        assert!(hub.senders.get("user-1").is_none());
    }
}
