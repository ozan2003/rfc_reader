//! Logging utilities
//!
//! This module provides utilities for logging to the console and a file.
//!
//! The logging system is configured to log to a file and the console.
//! The file is rotated daily and the log file is named `rfc_reader.log`.
//! The log file is stored in the data directory of the application.
//!
//! The log level is set to `info` and can be changed by setting the `RUST_LOG`
//! environment variable.

use std::path::PathBuf;

use directories::ProjectDirs;
use tracing_appender::rolling::RollingFileAppender;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    self, filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

/// Initialize the logging system
pub fn init_logging() -> anyhow::Result<()>
{
    let project_dirs = ProjectDirs::from("com", "rfc_reader", "rfc_reader")
        .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

    let log_dir = project_dirs.data_dir().join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("rfc_reader")
        .filename_suffix("log")
        .build(&log_dir)?;

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(file_appender)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("rfc_reader=trace"))
        .unwrap();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(ErrorLayer::default())
        .with(file_layer)
        .init();

    Ok(())
}

/// Get the path to the log directory
pub fn get_log_dir() -> anyhow::Result<PathBuf>
{
    let project_dirs = ProjectDirs::from("com", "rfc_reader", "rfc_reader")
        .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;
    Ok(project_dirs.data_dir().join("logs"))
}
