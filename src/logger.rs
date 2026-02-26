//! Debug logging support for SedX
//!
//! When debug mode is enabled via config, operations are logged to a file.
//! Logs are written to /var/log/sedx.log if writable, otherwise ~/.sedx/sedx.log

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

/// Initialize the debug logging system
///
/// If debug_enabled is true, sets up file logging.
/// Returns the path to the log file, or None if logging is not enabled.
pub fn init_debug_logging(debug_enabled: bool) -> Result<Option<PathBuf>> {
    if !debug_enabled {
        return Ok(None);
    }

    // Try /var/log/sedx.log first, fall back to ~/.sedx/sedx.log
    let log_path = get_log_path()?;

    // Ensure parent directory exists
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;
    }

    // Create the log file or append to existing
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("Failed to open log file: {}", log_path.display()));

    // If we can't open the log file, gracefully fall back to no logging
    match file {
        Ok(log_file) => {
            // Set up tracing subscriber with file output
            let subscriber = registry()
                .with(
                    fmt::layer()
                        .with_writer(log_file)
                        .with_ansi(false)
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_file(false)
                        .with_line_number(false)
                )
                .with(EnvFilter::new("sedx=info"));

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set tracing subscriber: {}", e))?;

            Ok(Some(log_path))
        }
        Err(e) => {
            // Silently fall back to no logging if we can't create the log file
            // This prevents breaking normal operation if logging fails
            eprintln!("Warning: Could not create log file: {}", e);
            Ok(None)
        }
    }
}

/// Get the log file path
///
/// Tries /var/log/sedx.log first, falls back to ~/.sedx/sedx.log
fn get_log_path() -> Result<PathBuf> {
    let var_log_path = PathBuf::from("/var/log/sedx.log");

    // Try to check if /var/log is writable
    if can_write_to_var_log() {
        return Ok(var_log_path);
    }

    // Fall back to home directory
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let sedx_dir = home_dir.join(".sedx");
    Ok(sedx_dir.join("sedx.log"))
}

/// Check if /var/log is writable
fn can_write_to_var_log() -> bool {
    // Try to create a test file in /var/log
    let test_file = "/var/log/.sedx_test_write";
    match fs::write(test_file, b"") {
        Ok(_) => {
            // Clean up test file
            let _ = fs::remove_file(test_file);
            true
        }
        Err(_) => false,
    }
}

/// Get the current log file path without initializing logging
///
/// This is used for the `sedx config --log-path` command
pub fn get_current_log_path() -> PathBuf {
    if can_write_to_var_log() {
        PathBuf::from("/var/log/sedx.log")
    } else {
        dirs::home_dir()
            .map(|h| h.join(".sedx/sedx.log"))
            .unwrap_or_else(|| PathBuf::from("~/.sedx/sedx.log"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_log_path() {
        let path = get_current_log_path();
        // Should return either /var/log/sedx.log or ~/.sedx/sedx.log
        #[allow(clippy::cmp_owned)]
        let is_var_log = path == PathBuf::from("/var/log/sedx.log");
        assert!(
            is_var_log || path.ends_with(".sedx/sedx.log"),
            "Log path should be either /var/log/sedx.log or in .sedx directory, got: {}",
            path.display()
        );
    }

    #[test]
    fn test_init_debug_logging_disabled() {
        let result = init_debug_logging(false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None, "Should return None when debug is disabled");
    }

    #[test]
    fn test_can_write_to_var_log() {
        // This test just verifies the function runs without panic
        // The actual result depends on the system running the tests
        let _can_write = can_write_to_var_log();
    }
}
