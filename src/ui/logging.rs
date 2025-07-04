//! Logging utilities
//!
//! This module provides functionality to initialize logging for the
//! application.

use std::fs::{File, create_dir_all, remove_file};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use anyhow::Result;
use directories::BaseDirs;
use env_logger::fmt::TimestampPrecision;
use env_logger::{Builder, Target};
use log::LevelFilter;

// Static log file path that can be accessed from other modules.
pub static LOG_FILE_PATH: LazyLock<Mutex<PathBuf>> = LazyLock::new(|| {
    let log_dir_path = BaseDirs::new()
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .expect("Failed to get cache directory");

    // Ensure log directory exists
    if !log_dir_path.exists()
    {
        create_dir_all(&log_dir_path).expect("Failed to create log directory");
    }

    Mutex::new(log_dir_path.join("rfc_reader.log"))
});

/// Initializes the logging system for the application.
///
/// This function sets up the logging configuration, including the
/// log file path, log level, and log format.
///
/// # Panics
///
/// Panics if the log file path cannot be locked.
///
/// # Errors
///
/// Returns an error if the log file cannot be opened or created.
pub fn init_logging() -> Result<()>
{
    // Use the static log file path
    let log_path = LOG_FILE_PATH.lock().unwrap();

    let log_file = File::options()
        .append(true)
        .create(true)
        .open(&*log_path)?;

    // Initialize the logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("rfc_reader", LevelFilter::Debug)
        .format_timestamp(Some(TimestampPrecision::Millis))
        .target(Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
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
    let log_path = LOG_FILE_PATH.lock().unwrap();

    if log_path.exists()
    {
        remove_file(&*log_path)?;
    }
    Ok(())
}
