//! Provides application logging utilities.
//!
//! Handles the initialization and configuration of the application's
//! logging system.
use std::fs::{OpenOptions, create_dir_all, read_dir, remove_file};
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use directories::BaseDirs;
use env_logger::{Builder, Target};
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use log::LevelFilter;

const LOG_FILE_SIZE: usize = 5 * 1024 * 1024; // 5MB
const MAX_LOG_FILE_COUNT: usize = 5;
const UNCOMPRESSED_LOG_FILE_COUNT: usize = 2;

// This is where the log file will be stored.
static LOG_FILE_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let base_dirs =
        BaseDirs::new().expect("Failed to determine base directories");

    let log_dir_path = if cfg!(target_os = "linux")
    {
        // SAFETY: This block is only executed if we are using Linux,
        // as the function only returns 'Some' here.
        unsafe {
            // Use `$XDG_STATE_HOME` on linux as its stated
            // on XDG Base Directory Specification.
            base_dirs.state_dir().unwrap_unchecked()
        }
    }
    else
    {
        base_dirs.data_local_dir()
    };

    if !log_dir_path.exists()
    {
        create_dir_all(log_dir_path).expect("Failed to create log directory");
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
    // static assertion to prevent skill issues in the future
    const {
        assert!(
            UNCOMPRESSED_LOG_FILE_COUNT < MAX_LOG_FILE_COUNT,
            "How can we compress more file than we have right now?"
        );
    }

    let log_path = get_log_file_path();

    let log_open_option = {
        let mut option = OpenOptions::new();
        option.read(true).create(true).append(true);

        option
    };

    let rotator = FileRotate::new(
        log_path,
        AppendCount::new(MAX_LOG_FILE_COUNT),
        ContentLimit::Bytes(LOG_FILE_SIZE),
        Compression::OnRotate(UNCOMPRESSED_LOG_FILE_COUNT),
        Some(log_open_option),
    );

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
        .target(Target::Pipe(Box::new(rotator)));

    builder
        .try_init()
        .context("Failed to initialize logger")
}

/// Removes the log files.
///
/// # Returns
///
/// Returns `Ok(())` if the files were successfully removed or didn't exist.
/// Returns an error if the files exist but couldn't be removed.
///
/// # Panics
///
/// Panics if the log files path cannot be locked.
///
/// # Errors
///
/// Returns an error if the files exist but couldn't be removed.
pub fn clear_log_files() -> Result<()>
{
    let log_path = get_log_file_path();

    if let Some(dir) = log_path.parent() &&
        let Some(log_name) = log_path.file_name().and_then(|s| s.to_str())
    {
        for entry in read_dir(dir).context("Failed to read log directory")?
        {
            let path = entry?.path();

            if !path.is_file()
            {
                continue;
            }

            let Some(name) = path.file_name().and_then(|s| s.to_str())
            else
            {
                continue;
            };

            if name == log_name || name.starts_with(&format!("{log_name}."))
            {
                remove_file(path).context("Failed to remove log file")?;
            }
        }
    }
    Ok(())
}
