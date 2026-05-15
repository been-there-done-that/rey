use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct InflightGuard {
    pub notify: Arc<Notify>,
    is_first: bool,
}

impl InflightGuard {
    pub fn is_waiter(&self) -> bool {
        !self.is_first
    }
}

pub struct InflightMap {
    map: DashMap<i64, Arc<Notify>>,
}

impl InflightMap {
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    pub fn get_or_insert(&self, file_id: i64) -> InflightGuard {
        if let Some(entry) = self.map.get(&file_id) {
            InflightGuard {
                notify: entry.value().clone(),
                is_first: false,
            }
        } else {
            let notify = Arc::new(Notify::new());
            self.map.insert(file_id, notify.clone());
            InflightGuard {
                notify,
                is_first: true,
            }
        }
    }

    pub fn remove_and_notify(&self, file_id: i64) {
        if let Some((_, notify)) = self.map.remove(&file_id) {
            notify.notify_waiters();
        }
    }
}

impl Default for InflightMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inflight_first_insert() {
        let inflight = InflightMap::new();
        let guard = inflight.get_or_insert(1);
        assert!(!guard.is_waiter());
    }

    #[test]
    fn test_inflight_second_is_waiter() {
        let inflight = InflightMap::new();
        let _guard1 = inflight.get_or_insert(1);
        let guard2 = inflight.get_or_insert(1);
        assert!(guard2.is_waiter());
    }

    #[test]
    fn test_inflight_remove_and_notify() {
        let inflight = InflightMap::new();
        let _guard1 = inflight.get_or_insert(1);
        inflight.remove_and_notify(1);
        let guard2 = inflight.get_or_insert(1);
        assert!(!guard2.is_waiter()); // should be first again after removal
    }
}
