//! Provides application logging utilities.
//!
//! Handles the initialization and configuration of the application's
//! logging system.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use env_logger::{Builder, Target};
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use log::LevelFilter;

const LOG_FILE_SIZE: usize = 5 * 1024 * 1024; // 5MB
const MAX_LOG_FILE_COUNT: usize = 5;
const UNCOMPRESSED_LOG_FILE_COUNT: usize = 2;

/// This is directory where the log files will be stored.
static LOG_FILES_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let base_dirs =
        BaseDirs::new().expect("Failed to determine base directories");

    let base_path = if cfg!(target_os = "linux")
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

    fs::create_dir_all(base_path).expect("Failed to create base directory");

    // Use a dedicated directory for logs
    let logs_dir_path = base_path
        .join(env!("CARGO_PKG_NAME"))
        .join("logs");

    fs::create_dir_all(&logs_dir_path).expect("Failed to create log directory");

    logs_dir_path.into_boxed_path()
});

/// Base log file path inside the logs directory.
///
/// Formatted as: `<package-name>.log`
static BASE_LOG_FILE_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    get_log_files_dir_path()
        .join(concat!(env!("CARGO_PKG_NAME"), ".log"))
        .into_boxed_path()
});

/// Returns the path to the directory where the log files are stored.
///
/// # Returns
///
/// A static `Path` reference to the log files directory path.
#[must_use]
pub fn get_log_files_dir_path() -> &'static Path
{
    &LOG_FILES_PATH
}

/// Returns the path to the log file.
///
/// # Returns
///
/// A static `Path` reference to the log file path.
#[must_use]
fn get_base_log_file_path() -> &'static Path
{
    &BASE_LOG_FILE_PATH
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

    let base_log_file_path = get_base_log_file_path();

    let log_open_option = {
        let mut option = OpenOptions::new();
        option.read(true).create(true).append(true);

        option
    };

    // Files are rotated as `<package-name>.log.<count>`
    let rotator = FileRotate::new(
        base_log_file_path,
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
/// # Errors
///
/// Returns an error if the files exist but couldn't be removed.
pub fn clear_log_files() -> Result<()>
{
    let log_files_path = get_log_files_dir_path();

    if !log_files_path.exists()
    {
        return Ok(());
    }

    let Some(base_log_name) = get_base_log_file_path()
        .file_name()
        .and_then(|s| s.to_str())
    else
    {
        bail!("Failed to get log file name");
    };

    for entry in fs::read_dir(log_files_path)
        .context("Failed to read log files directory")?
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

        if name == base_log_name ||
            name.strip_prefix(base_log_name)
                .is_some_and(|s| s.starts_with('.'))
        {
            fs::remove_file(path).context("Failed to remove log file")?;
        }
    }

    // Remove logs directory if empty, then app directory if it also
    // became empty.
    if fs::remove_dir(log_files_path).is_ok() &&
        let Some(app_dir) = log_files_path.parent()
    {
        let _ = fs::remove_dir(app_dir);
    }

    Ok(())
}
