//! Provides application logging utilities.
//!
//! Handles the initialization and configuration of the application's
//! logging system.
use std::fs::{File, create_dir_all, remove_file};
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use directories::BaseDirs;
use env_logger::{Builder, Target};
use log::LevelFilter;

// This is where the log file will be stored.
static LOG_FILE_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let base_dirs =
        BaseDirs::new().expect("Failed to determine base directories");
    let log_dir_path = base_dirs.cache_dir().to_path_buf();

    if !log_dir_path.exists()
    {
        create_dir_all(&log_dir_path).expect("Failed to create log directory");
    }

    log_dir_path
        .join(concat!(env!("CARGO_PKG_NAME"), ".log"))
        .into_boxed_path()
});

/// Returns the path to the log file.
///
/// # Returns
///
/// A static `Path` reference to the log file path.
#[must_use]
pub fn get_log_file_path() -> &'static Path
{
    &LOG_FILE_PATH
}

/// Initializes the logging system for the application.
///
/// This function sets up the logging configuration, including the
/// log file path, log level, and log format.
///
/// # Errors
///
/// Returns an error if the log file cannot be opened or created.
pub fn init_logging() -> Result<()>
{
    let log_path = get_log_file_path();

    let log_file = File::options()
        .append(true)
        .create(true)
        .open(log_path)?;

    let mut builder = Builder::new();
    builder
        .filter_level(LevelFilter::Info)
        .filter_module(env!("CARGO_PKG_NAME"), LevelFilter::Debug)
        .parse_default_env()
        .format(|buf, record| {
            let ts = buf.timestamp_millis();
            writeln!(
                buf,
                "{ts} {:<5} {}: {}",
                record.level(),
                record.target(),
                record.args()
            )
        })
        .target(Target::Pipe(Box::new(log_file)));

    builder
        .try_init()
        .context("Failed to initialize logger")
}

/// Removes the log file.
///
/// # Returns
///
/// Returns `Ok(())` if the file was successfully removed or didn't exist.
/// Returns an error if the file exists but couldn't be removed.
///
/// # Panics
///
/// Panics if the log file path cannot be locked.
///
/// # Errors
///
/// Returns an error if the file exists but couldn't be removed.
pub fn clear_log_file() -> Result<()>
{
    let log_path = get_log_file_path();

    if log_path.exists()
    {
        remove_file(log_path)?;
    }
    Ok(())
}
