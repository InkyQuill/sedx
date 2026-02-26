//! Cross-platform disk space checking
//!
//! Provides functionality to check available disk space before creating backups

use anyhow::{Context, Result};
use std::path::Path;

/// Information about disk space usage
#[derive(Debug, Clone)]
pub struct DiskSpaceInfo {
    /// Total disk space in bytes
    pub total_bytes: u64,
    /// Available disk space in bytes
    pub available_bytes: u64,
    /// Used disk space in bytes
    #[allow(dead_code)] // Kept for API completeness and potential future use
    pub used_bytes: u64,
    /// Percentage of disk used
    #[allow(dead_code)] // Kept for API completeness and potential future use
    pub used_percent: f64,
}

impl DiskSpaceInfo {
    /// Convert bytes to human-readable format (e.g., "1.5 GB")
    pub fn bytes_to_human(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        const TB: u64 = 1024 * GB;

        if bytes >= TB {
            format!("{:.1} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Get available space in human-readable format
    pub fn available_human(&self) -> String {
        Self::bytes_to_human(self.available_bytes)
    }

    /// Get total space in human-readable format
    pub fn total_human(&self) -> String {
        Self::bytes_to_human(self.total_bytes)
    }
}

/// Check available disk space for a given path
///
/// # Arguments
/// * `path` - Path to check (typically the backup directory or file location)
///
/// # Returns
/// `DiskSpaceInfo` with disk usage statistics
///
/// # Platform support
/// - Linux/macOS: Uses `statvfs` system call
/// - Windows: Uses `GetDiskFreeSpaceEx` via windows-rs (not yet implemented)
#[cfg(unix)]
pub fn get_disk_space(path: &Path) -> Result<DiskSpaceInfo> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // Convert path to CString for statvfs
    let c_path =
        CString::new(path.as_os_str().as_bytes()).context("Failed to convert path to CString")?;

    // Get statvfs structure
    // # Safety
    //
    // `std::mem::zeroed()` is safe for `libc::statvfs` because it's a C struct
    // containing only primitive integer types and arrays of integers.
    // `libc::statvfs` is a POSIX system call that writes to the provided mutable reference.
    // The `c_path` pointer is valid because it comes from a `CString` whose lifetime
    // exceeds this function call. Return value is checked for errors.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };

    unsafe {
        if libc::statvfs(c_path.as_ptr(), &mut stat) != 0 {
            return Err(anyhow::anyhow!(
                "Failed to get disk space for '{}': {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
    }

    // Calculate values from statvfs
    let frsize = stat.f_frsize;
    let total_bytes = stat.f_blocks * frsize;
    let available_bytes = stat.f_bavail * frsize;
    let used_bytes = total_bytes - available_bytes;
    let used_percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(DiskSpaceInfo {
        total_bytes,
        available_bytes,
        used_bytes,
        used_percent,
    })
}

/// Check disk space for a given path (Windows stub)
///
/// Windows implementation not yet available - always returns error
#[cfg(windows)]
pub fn get_disk_space(_path: &Path) -> Result<DiskSpaceInfo> {
    Err(anyhow::anyhow!(
        "Windows disk space checking not yet implemented. \
        Please use Linux or macOS, or disable disk space checks."
    ))
}

/// Check if there's enough disk space for a backup
///
/// # Arguments
/// * `backup_dir` - Directory where backup will be created
/// * `file_size` - Size of the file to be backed up in bytes
/// * `max_percent` - Maximum percentage of free space to use (default: 60)
///
/// # Returns
/// `Ok(())` if there's enough space, `Err` otherwise
pub fn check_disk_space_for_backup(
    backup_dir: &Path,
    file_size: u64,
    max_percent: f64,
) -> Result<()> {
    let space = get_disk_space(backup_dir).context("Failed to check disk space")?;

    // Calculate what percentage of free space the backup would use
    let percent_of_free = if space.available_bytes > 0 {
        (file_size as f64 / space.available_bytes as f64) * 100.0
    } else {
        100.0 // No space available
    };

    // Check if backup would exceed max percent of free space
    if percent_of_free > max_percent {
        return Err(anyhow::anyhow!(
            "Insufficient disk space for backup\n\
             backup partition: {}\n\
             available: {} (total: {})\n\
             backup required: {} ({:.1}% of free space)\n\
             maximum allowed: {:.1}% of free space\n\
             \n\
             Options:\n\
             1. Remove old backups: sedx backup prune --keep=5\n\
             2. Use different location: --backup-dir /mnt/backups\n\
             3. Skip backup: --no-backup --force (not recommended)",
            backup_dir.display(),
            space.available_human(),
            space.total_human(),
            DiskSpaceInfo::bytes_to_human(file_size),
            percent_of_free,
            max_percent
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_get_disk_space() {
        let space = get_disk_space(Path::new("/"));
        assert!(space.is_ok());

        let space = space.unwrap();
        assert!(space.total_bytes > 0);
        assert!(space.available_bytes > 0);
        assert!(space.used_percent >= 0.0 && space.used_percent <= 100.0);
    }

    #[test]
    fn test_bytes_to_human() {
        assert_eq!(DiskSpaceInfo::bytes_to_human(500), "500 B");
        assert_eq!(DiskSpaceInfo::bytes_to_human(1024), "1.0 KB");
        assert_eq!(DiskSpaceInfo::bytes_to_human(1024 * 1024), "1.0 MB");
        assert_eq!(DiskSpaceInfo::bytes_to_human(1024 * 1024 * 1024), "1.0 GB");
    }
}
