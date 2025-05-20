//! Logging utilities
//!
//! This module provides functionality to initialize logging for the
//! application.

use directories::BaseDirs;
use env_logger::{Builder, Target, fmt::TimestampPrecision};
use log::LevelFilter;
use std::fs::{File, remove_file};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

// Static log file path that can be accessed from other modules
pub static LOG_FILE: LazyLock<Mutex<PathBuf>> = LazyLock::new(|| {
    let binding = BaseDirs::new().unwrap();
    let log_dir = binding.cache_dir();
    Mutex::new(log_dir.join("rfc_reader.log"))
});

/// Initializes the logging system for the application.
///
/// This function sets up the logging configuration, including the
/// log file path, log level, and log format.
///
/// # Panics
///
/// Panics if the log file cannot be opened or created.
pub fn init_logging()
{
    // Use the static log file path
    let log_path = LOG_FILE.lock().unwrap();

    let log_file = File::options()
        .append(true)
        .create(true)
        .open(&*log_path)
        .expect("Failed to open log file");

    // Initialize the logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("rfc_reader", LevelFilter::Debug)
        .format_timestamp(Some(TimestampPrecision::Millis))
        .target(Target::Pipe(Box::new(log_file)))
        .init();
}

/// Removes the log file.
///
/// # Panics
///
/// Panics if the log file lock cannot be acquired or if the file
/// removal fails.
pub fn clear_log_file()
{
    let log_path = LOG_FILE.lock().unwrap();

    remove_file(&*log_path).unwrap();
}
