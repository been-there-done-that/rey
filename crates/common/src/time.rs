use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

pub fn now_utc() -> SystemTime {
    SystemTime::now()
}

pub fn from_ms(ms: i64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_millis(ms as u64)
}

pub fn to_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .expect("time is before Unix epoch")
        .as_millis() as i64
}

pub fn elapsed_ms(since_ms: i64) -> Option<i64> {
    let now = now_ms();
    if now >= since_ms {
        Some(now - since_ms)
    } else {
        None
    }
}

pub fn is_older_than(ms: i64, seconds: i64) -> bool {
    elapsed_ms(ms)
        .map(|elapsed| elapsed > seconds * 1000)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_ms_returns_reasonable_timestamp() {
        let ts = now_ms();
        assert!(ts > 1_700_000_000_000);
        assert!(ts < 4_102_444_800_000);
    }

    #[test]
    fn test_now_ms_is_monotonically_increasing() {
        let first = now_ms();
        let second = now_ms();
        assert!(second >= first);
    }

    #[test]
    fn test_from_ms_to_ms_roundtrip() {
        let original = 1_700_000_000_000i64;
        let time = from_ms(original);
        let recovered = to_ms(time);
        assert_eq!(recovered, original);
    }

    #[test]
    fn test_elapsed_ms_returns_some_for_past_timestamp() {
        let past = now_ms() - 5000;
        let elapsed = elapsed_ms(past);
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap() >= 0);
    }

    #[test]
    fn test_elapsed_ms_returns_none_for_future_timestamp() {
        let future = now_ms() + 10_000_000;
        let elapsed = elapsed_ms(future);
        assert!(elapsed.is_none());
    }

    #[test]
    fn test_is_older_than_true_for_old_timestamp() {
        let old = now_ms() - 10_000;
        assert!(is_older_than(old, 5));
    }

    #[test]
    fn test_is_older_than_false_for_recent_timestamp() {
        let recent = now_ms();
        assert!(!is_older_than(recent, 5));
    }
}
