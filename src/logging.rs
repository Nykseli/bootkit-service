use std::str::FromStr;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::config::DEFAULT_LOG_LEVEL;

fn log_level() -> tracing::Level {
    if let Some(level) = option_env!("BOOTKIT_LOG_LEVEL") {
        // Possible values (case insensitive)
        // ERROR
        // WARN
        // INFO
        // DEBUG
        // TRACE
        tracing::Level::from_str(level).unwrap_or(DEFAULT_LOG_LEVEL)
    } else {
        DEFAULT_LOG_LEVEL
    }
}

pub fn setup_logging() {
    let level = log_level();

    let filter = Targets::new().with_default(level);
    // turn off library logging unless we want to trace everything
    let filter = if level != tracing::Level::TRACE {
        filter
            .with_target("sqlx", LevelFilter::OFF)
            .with_target("zbus", LevelFilter::OFF)
    } else {
        filter
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .init();
}
