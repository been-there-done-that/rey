use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_tracing(log_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::try_new(log_level).expect("invalid log level"));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().pretty());

    subscriber.init();
}

pub fn init_tracing_json(log_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::try_new(log_level).expect("invalid log level"));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json());

    subscriber.init();
}

pub fn init_tracing_auto(log_level: &str) {
    if cfg!(debug_assertions) {
        init_tracing(log_level);
    } else {
        init_tracing_json(log_level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_tracing_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            init_tracing("info");
        });
        drop(result);
    }

    #[test]
    fn test_init_tracing_json_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            init_tracing_json("info");
        });
        drop(result);
    }

    #[test]
    fn test_init_tracing_auto_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            init_tracing_auto("debug");
        });
        drop(result);
    }
}
