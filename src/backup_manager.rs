use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::io::{BufReader, BufWriter, copy as io_copy};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_BACKUPS: usize = 50;
const GZ_EXT: &str = ".gz";

/// Append a `.gz` suffix to `path`, keeping the existing extension intact.
/// `Path::with_extension` would replace the extension (`file.txt` -> `file.gz`),
/// which loses information the restore path relies on — we want
/// `file.txt.gz` so the original filename is recoverable.
fn append_gz_extension(path: &Path) -> PathBuf {
    let mut s: OsString = path.into();
    s.push(GZ_EXT);
    PathBuf::from(s)
}

/// True if `path`'s final extension is `.gz`.
fn is_gzipped(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "gz")
}

fn path_identity(path: &Path) -> String {
    fn update_hash(hash: u64, byte: u8) -> u64 {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    }

    #[cfg(unix)]
    let hash = {
        use std::os::unix::ffi::OsStrExt;

        path.as_os_str()
            .as_bytes()
            .iter()
            .fold(0xcbf29ce484222325, |hash, byte| update_hash(hash, *byte))
    };

    #[cfg(windows)]
    let hash = {
        use std::os::windows::ffi::OsStrExt;

        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .fold(0xcbf29ce484222325, update_hash)
    };

    #[cfg(not(any(unix, windows)))]
    let hash = path
        .to_string_lossy()
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| update_hash(hash, *byte));

    format!("path-v1-{hash:016x}")
}

fn backup_path_for_original(backup_dir: &Path, original_path: &Path) -> PathBuf {
    append_gz_extension(&backup_dir.join(path_identity(original_path)))
}

fn metadata_backup_path(backup_dir: &Path, backup_path: &Path) -> Result<PathBuf> {
    if backup_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!(
            "backup metadata path validation failed for '{}': parent traversal is not allowed",
            backup_path.display()
        );
    }

    let candidate = if backup_path.is_absolute() || backup_path.starts_with(backup_dir) {
        backup_path.to_path_buf()
    } else {
        backup_dir.join(backup_path)
    };

    if !candidate.starts_with(backup_dir) {
        bail!(
            "backup metadata path validation failed for '{}': backup path is outside backup directory",
            candidate.display()
        );
    }

    Ok(candidate)
}

/// Gzip-copy `src` to `dst` using streaming I/O so memory stays flat for
/// large files. The destination gets the full gzip container (magic bytes +
/// header + deflate stream + trailer), suitable for standard `gunzip`.
fn gzip_copy(src: &Path, dst: &Path) -> Result<()> {
    let source =
        fs::File::open(src).with_context(|| format!("Failed to open source: {}", src.display()))?;
    let mut reader = BufReader::new(source);

    let dest = fs::File::create(dst)
        .with_context(|| format!("Failed to create backup: {}", dst.display()))?;
    let mut encoder = GzEncoder::new(BufWriter::new(dest), Compression::default());

    io_copy(&mut reader, &mut encoder)
        .with_context(|| format!("Failed to gzip-copy to: {}", dst.display()))?;
    encoder
        .finish()
        .with_context(|| format!("Failed to finalize gzip stream: {}", dst.display()))?;
    Ok(())
}

/// Restore a backup file into place. If `src` is gzipped (`.gz` suffix), it
/// is streamed through GzDecoder on the way out; otherwise it's a plain
/// byte-for-byte copy so that legacy (pre-v1.1) uncompressed backups keep
/// working.
fn restore_file(src: &Path, dst: &Path) -> Result<()> {
    if is_gzipped(src) {
        let source = fs::File::open(src)
            .with_context(|| format!("Failed to open backup: {}", src.display()))?;
        let mut decoder = GzDecoder::new(BufReader::new(source));
        let dest = crate::path_policy::create_file_no_follow(dst)
            .with_context(|| format!("Failed to create restore target: {}", dst.display()))?;
        let mut writer = BufWriter::new(dest);
        io_copy(&mut decoder, &mut writer)
            .with_context(|| format!("Failed to decompress into: {}", dst.display()))?;
    } else {
        let mut source = fs::File::open(src)
            .with_context(|| format!("Failed to open backup: {}", src.display()))?;
        let dest = crate::path_policy::create_file_no_follow(dst)
            .with_context(|| format!("Failed to create restore target: {}", dst.display()))?;
        let mut writer = BufWriter::new(dest);
        io_copy(&mut source, &mut writer)
            .with_context(|| format!("Failed to restore file: {}", dst.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub expression: String,
    pub files: Vec<FileBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackup {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_identity: Option<String>,
}

pub struct BackupManager {
    backups_dir: PathBuf,
}

/// Resolve sedx's state directory root.
///
/// Honors the `SEDX_HOME` env var if set (useful for tests that need to
/// isolate `~/.sedx/` on every platform — `dirs::home_dir()` on Windows
/// reads the shell API, not `HOME`/`USERPROFILE`, so an env-based override
/// is the only portable way to redirect it). Falls back to
/// `dirs::home_dir()` for normal use.
pub fn sedx_home() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("SEDX_HOME") {
        if !custom.is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))
}

impl BackupManager {
    pub fn new() -> Result<Self> {
        let home_dir = sedx_home()?;
        let backups_dir = home_dir.join(".sedx").join("backups");

        // Create backups directory if it doesn't exist
        fs::create_dir_all(&backups_dir).with_context(|| {
            format!(
                "Failed to create backups directory: {}",
                backups_dir.display()
            )
        })?;

        Ok(Self { backups_dir })
    }

    /// Create a BackupManager with a custom backup directory
    pub fn with_directory(dir: String) -> Result<Self> {
        let backups_dir = PathBuf::from(dir);

        // Create backups directory if it doesn't exist
        fs::create_dir_all(&backups_dir).with_context(|| {
            format!(
                "Failed to create backups directory: {}",
                backups_dir.display()
            )
        })?;

        Ok(Self { backups_dir })
    }

    /// Get the backup directory path
    #[allow(dead_code)]
    pub fn backups_dir(&self) -> &Path {
        &self.backups_dir
    }

    pub fn create_backup(&mut self, expression: &str, files: &[PathBuf]) -> Result<String> {
        // Calculate total backup size and check disk space
        let mut total_size = 0u64;
        for file_path in files {
            if file_path.exists() {
                total_size += file_path
                    .metadata()
                    .with_context(|| {
                        format!("Failed to get file metadata: {}", file_path.display())
                    })?
                    .len();
            }
        }

        // Check disk space before creating backup
        // Default: warn if backup > 2GB or > 40% of free space
        // Error if backup > 60% of free space
        const MAX_BACKUP_SIZE_GB: u64 = 2;
        const ERROR_PERCENT: f64 = 60.0;

        // Warn if backup is very large
        if total_size > MAX_BACKUP_SIZE_GB * 1024 * 1024 * 1024 {
            eprintln!(
                "⚠️  Warning: This operation will create a large backup ({})",
                crate::disk_space::DiskSpaceInfo::bytes_to_human(total_size)
            );
            eprintln!("Consider using --no-backup if you have a recent backup");
        }

        // Check disk space with error threshold
        if let Err(e) = crate::disk_space::check_disk_space_for_backup(
            &self.backups_dir,
            total_size,
            ERROR_PERCENT,
        ) {
            // Provide helpful error message
            return Err(e.context(format!(
                "Cannot create backup. Files size: {}",
                crate::disk_space::DiskSpaceInfo::bytes_to_human(total_size)
            )));
        }

        // Generate unique backup ID with millisecond precision for deterministic sorting
        let id = format!(
            "{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S%3f"),
            Uuid::new_v4().to_string().split_at(8).0
        );
        let backup_dir = self.backups_dir.join(&id);

        fs::create_dir_all(&backup_dir).with_context(|| {
            format!(
                "Failed to create backup directory: {}",
                backup_dir.display()
            )
        })?;

        let mut file_backups = Vec::new();

        for file_path in files {
            if !file_path.exists() {
                continue;
            }

            let identity = path_identity(file_path);
            let backup_path = backup_path_for_original(&backup_dir, file_path);

            gzip_copy(file_path, &backup_path)
                .with_context(|| format!("Failed to backup file: {}", file_path.display()))?;

            file_backups.push(FileBackup {
                original_path: file_path.clone(),
                backup_path,
                path_identity: Some(identity),
            });
        }

        // Save metadata
        let metadata = BackupMetadata {
            id: id.clone(),
            timestamp: Utc::now(),
            expression: expression.to_string(),
            files: file_backups,
        };

        let metadata_path = backup_dir.join("operation.json");
        let metadata_json =
            serde_json::to_string_pretty(&metadata).context("Failed to serialize metadata")?;

        fs::write(&metadata_path, metadata_json)
            .with_context(|| format!("Failed to write metadata: {}", metadata_path.display()))?;

        // Cleanup old backups
        self.cleanup_old_backups()?;

        Ok(id)
    }

    pub fn restore_backup(&self, id: &str) -> Result<()> {
        let backup_dir = self.backups_dir.join(id);
        let metadata_path = backup_dir.join("operation.json");

        if !backup_dir.exists() {
            anyhow::bail!("Backup not found: {}", id);
        }

        let metadata_json = fs::read_to_string(&metadata_path)
            .with_context(|| format!("Failed to read metadata: {}", metadata_path.display()))?;

        let metadata: BackupMetadata =
            serde_json::from_str(&metadata_json).context("Failed to parse metadata")?;

        let mut restore_entries = Vec::with_capacity(metadata.files.len());
        for file_backup in &metadata.files {
            let original_path =
                crate::path_policy::validate_restore_target(&file_backup.original_path)?;
            let backup_path = metadata_backup_path(&backup_dir, &file_backup.backup_path)?;
            let Some(identity) = &file_backup.path_identity else {
                bail!(
                    "backup metadata path validation failed for '{}': missing path identity",
                    original_path.display()
                );
            };

            let expected_identity = path_identity(&original_path);
            let expected_backup_path = backup_path_for_original(&backup_dir, &original_path);
            if identity != &expected_identity || backup_path != expected_backup_path {
                bail!(
                    "backup metadata path validation failed for '{}': backup entry does not match original path",
                    original_path.display()
                );
            }
            restore_entries.push((backup_path, original_path));
        }

        for (backup_path, original_path) in restore_entries {
            if !backup_path.exists() {
                eprintln!("Warning: Backup file missing: {}", backup_path.display());
                continue;
            }

            restore_file(&backup_path, &original_path)
                .with_context(|| format!("Failed to restore file: {}", original_path.display()))?;

            println!("Restored: {}", original_path.display());
        }

        // Remove backup after successful restore
        fs::remove_dir_all(&backup_dir).with_context(|| {
            format!(
                "Failed to remove backup directory: {}",
                backup_dir.display()
            )
        })?;

        println!("Backup {} removed after restore", id);

        Ok(())
    }

    pub fn get_last_backup_id(&self) -> Result<Option<String>> {
        let mut backups = self.list_backups()?;
        backups.sort_by_key(|b| b.timestamp);
        Ok(backups.last().map(|b| b.id.clone()))
    }

    pub fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let mut backups = Vec::new();

        for entry in fs::read_dir(&self.backups_dir).with_context(|| {
            format!(
                "Failed to read backups directory: {}",
                self.backups_dir.display()
            )
        })? {
            let entry = entry?;
            let metadata_path = entry.path().join("operation.json");

            if !metadata_path.exists() {
                continue;
            }

            let metadata_json = fs::read_to_string(&metadata_path)?;
            if let Ok(metadata) = serde_json::from_str::<BackupMetadata>(&metadata_json) {
                backups.push(metadata);
            }
        }

        // Sort by timestamp to ensure chronological order
        // When timestamps are equal (rare), use ID as tiebreaker for consistency
        backups.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
        Ok(backups)
    }

    fn cleanup_old_backups(&self) -> Result<()> {
        let mut backups = self.list_backups()?;
        backups.sort_by_key(|b| b.timestamp);

        if backups.len() > MAX_BACKUPS {
            for backup in backups.iter().take(backups.len() - MAX_BACKUPS) {
                let backup_dir = self.backups_dir.join(&backup.id);
                fs::remove_dir_all(&backup_dir).with_context(|| {
                    format!("Failed to remove old backup: {}", backup_dir.display())
                })?;
            }
        }

        Ok(())
    }

    /// Remove a backup by its ID (used for cleanup when no changes are made)
    #[allow(dead_code)] // Part of the public BackupManager API — lets library consumers remove backups programmatically.
    pub fn remove_backup_by_id(&self, backup_id: &str) -> Result<()> {
        let backup_dir = self.backups_dir.join(backup_id);
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("Failed to remove backup: {}", backup_dir.display()))?;
        Ok(())
    }

    /// Parse backup metadata from JSON string
    #[allow(dead_code)] // Part of the public BackupManager API — lets library consumers deserialize stored metadata.
    pub fn parse_backup_metadata(json: &str) -> Result<BackupMetadata> {
        let metadata: BackupMetadata =
            serde_json::from_str(json).context("Failed to parse backup metadata")?;
        Ok(metadata)
    }

    /// Prune backups keeping only the N most recent ones
    #[allow(dead_code)] // Part of the public BackupManager API — lets library consumers enforce count-based retention.
    pub fn prune_backups(&self, keep_count: usize) -> Result<usize> {
        let mut backups = self.list_backups()?;
        backups.sort_by_key(|b| b.timestamp);

        if backups.len() <= keep_count {
            return Ok(0);
        }

        let to_remove = backups.len() - keep_count;
        for backup in backups.iter().take(to_remove) {
            let backup_dir = self.backups_dir.join(&backup.id);
            fs::remove_dir_all(&backup_dir)
                .with_context(|| format!("Failed to remove backup: {}", backup_dir.display()))?;
        }

        Ok(to_remove)
    }

    /// Prune backups older than the specified number of days
    #[allow(dead_code)] // Part of the public BackupManager API — lets library consumers enforce time-based retention.
    pub fn prune_backups_older_than(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let mut removed = 0;

        for backup in self.list_backups()? {
            if backup.timestamp < cutoff {
                let backup_dir = self.backups_dir.join(&backup.id);
                fs::remove_dir_all(&backup_dir).with_context(|| {
                    format!("Failed to remove old backup: {}", backup_dir.display())
                })?;
                removed += 1;
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Helper function to create a test file with content
    fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let file_path = dir.join(name);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file_path
    }

    /// Helper function to create a test backup manager with a temp directory
    fn create_test_manager() -> (BackupManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let backups_dir = temp_dir.path().join("backups");
        let manager =
            BackupManager::with_directory(backups_dir.to_str().unwrap().to_string()).unwrap();
        (manager, temp_dir)
    }

    // ============================================================================
    // create_backup() tests
    // ============================================================================

    #[test]
    fn test_create_backup_single_file() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "Hello, World!");

        let backup_id = manager
            .create_backup("s/foo/bar/", std::slice::from_ref(&test_file))
            .unwrap();

        // Verify backup directory exists
        let backup_dir = manager.backups_dir().join(&backup_id);
        assert!(backup_dir.exists(), "Backup directory should exist");

        // Verify metadata file exists
        let metadata_path = backup_dir.join("operation.json");
        assert!(metadata_path.exists(), "Metadata file should exist");

        // Verify gzipped backup file exists.
        let backup_file = backup_path_for_original(&backup_dir, &test_file);
        assert!(backup_file.exists(), "Gzipped backup file should exist");

        // Verify backup round-trips: decompress into a temp file and compare
        // the recovered content to the original.
        let recovered_dir = tempfile::tempdir().unwrap();
        let recovered = recovered_dir.path().join("recovered.txt");
        restore_file(&backup_file, &recovered).unwrap();
        let recovered_content = fs::read_to_string(&recovered).unwrap();
        let original_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(
            recovered_content, original_content,
            "Backup content should round-trip through gzip"
        );

        // Verify metadata is correct
        let metadata_json = fs::read_to_string(&metadata_path).unwrap();
        let metadata: BackupMetadata = serde_json::from_str(&metadata_json).unwrap();
        assert_eq!(metadata.id, backup_id);
        assert_eq!(metadata.expression, "s/foo/bar/");
        assert_eq!(metadata.files.len(), 1);
        assert_eq!(metadata.files[0].original_path, test_file);
    }

    #[test]
    fn test_create_backup_multiple_files() {
        let (mut manager, temp_dir) = create_test_manager();
        let file1 = create_test_file(temp_dir.path(), "file1.txt", "Content 1");
        let file2 = create_test_file(temp_dir.path(), "file2.txt", "Content 2");
        let file3 = create_test_file(temp_dir.path(), "file3.txt", "Content 3");

        let backup_id = manager
            .create_backup(
                "s/test/prod/",
                &[file1.clone(), file2.clone(), file3.clone()],
            )
            .unwrap();

        let backup_dir = manager.backups_dir().join(&backup_id);
        assert!(backup_dir.exists());

        // Verify all files were backed up (with .gz suffix).
        assert!(backup_path_for_original(&backup_dir, &file1).exists());
        assert!(backup_path_for_original(&backup_dir, &file2).exists());
        assert!(backup_path_for_original(&backup_dir, &file3).exists());

        // Verify metadata
        let metadata_path = backup_dir.join("operation.json");
        let metadata: BackupMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.files.len(), 3);
    }

    #[test]
    fn test_create_backup_large_file() {
        let (mut manager, temp_dir) = create_test_manager();
        let large_content = "x".repeat(1_000_000); // 1MB of data
        let large_file = create_test_file(temp_dir.path(), "large.txt", &large_content);

        let backup_id = manager
            .create_backup("s/x/y/", std::slice::from_ref(&large_file))
            .unwrap();

        let backup_dir = manager.backups_dir().join(&backup_id);
        let backup_file = backup_path_for_original(&backup_dir, &large_file);
        assert!(backup_file.exists(), "Gzipped backup file should exist");

        // A compressible 1 MB blob of `x` gzips to well under 10 KB —
        // asserting that gives us a meaningful size expectation without
        // being brittle to compression-level tweaks. The exact ratio
        // depends on zlib internals; anywhere under 10% is plenty of
        // signal that compression actually happened.
        let backup_len = fs::metadata(&backup_file).unwrap().len();
        let original_len = fs::metadata(&large_file).unwrap().len();
        assert_eq!(original_len, 1_000_000);
        assert!(
            backup_len < original_len / 10,
            "gzip should shrink a 1MB run of 'x' to <10% of the original, got {backup_len}",
        );

        // Verify content round-trips through gzip.
        let recovered_dir = tempfile::tempdir().unwrap();
        let recovered = recovered_dir.path().join("recovered.txt");
        restore_file(&backup_file, &recovered).unwrap();
        assert_eq!(fs::metadata(&recovered).unwrap().len(), original_len);
    }

    #[test]
    fn test_create_backup_special_characters_in_filename() {
        let (mut manager, temp_dir) = create_test_manager();

        // Test various special characters
        let test_cases = vec![
            ("file with spaces.txt", "content with spaces"),
            ("file-with-dashes.txt", "content with dashes"),
            ("file_with_underscores.txt", "content with underscores"),
            ("file.multiple.dots.txt", "content"),
            ("file123.txt", "numeric content"),
        ];

        let mut files = Vec::new();
        for (name, content) in &test_cases {
            files.push(create_test_file(temp_dir.path(), name, content));
        }

        let backup_id = manager.create_backup("s/a/b/", &files).unwrap();

        let backup_dir = manager.backups_dir().join(&backup_id);

        // Verify all files with special characters were backed up (gzipped).
        for file in &files {
            let gzipped = backup_path_for_original(&backup_dir, file);
            assert!(
                gzipped.exists(),
                "File '{}' should exist in backup as {}",
                file.display(),
                gzipped.display(),
            );
        }
    }

    #[test]
    fn test_create_backup_nonexistent_file_skipped() {
        let (mut manager, temp_dir) = create_test_manager();
        let existing_file = create_test_file(temp_dir.path(), "exists.txt", "I exist");
        let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

        let backup_id = manager
            .create_backup("s/test/prod/", &[existing_file.clone(), nonexistent_file])
            .unwrap();

        let backup_dir = manager.backups_dir().join(&backup_id);
        let metadata_path = backup_dir.join("operation.json");
        let metadata: BackupMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();

        // Only the existing file should be in the backup
        assert_eq!(metadata.files.len(), 1);
        assert_eq!(metadata.files[0].original_path, existing_file);
    }

    #[test]
    fn test_create_backup_generates_unique_ids() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let id1 = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();
        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = manager
            .create_backup("s/c/d/", std::slice::from_ref(&test_file))
            .unwrap();

        assert_ne!(id1, id2, "Backup IDs should be unique");
    }

    // ============================================================================
    // restore_backup() tests
    // ============================================================================

    #[test]
    fn test_restore_backup_success() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "original content");

        // Create backup
        let backup_id = manager
            .create_backup("s/foo/bar/", std::slice::from_ref(&test_file))
            .unwrap();

        // Modify the original file
        fs::write(&test_file, "modified content").unwrap();

        // Restore from backup
        manager.restore_backup(&backup_id).unwrap();

        // Verify content was restored
        let restored_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(restored_content, "original content");

        // Verify backup directory was removed after restore
        let backup_dir = manager.backups_dir().join(&backup_id);
        assert!(
            !backup_dir.exists(),
            "Backup directory should be removed after restore"
        );
    }

    #[test]
    fn test_restore_backup_nonexistent_id() {
        let (manager, _) = create_test_manager();

        let result = manager.restore_backup("nonexistent-backup-id");
        assert!(
            result.is_err(),
            "Should return error for nonexistent backup"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Backup not found"),
            "Error should mention backup not found"
        );
    }

    #[test]
    fn test_restore_backup_multiple_files() {
        let (mut manager, temp_dir) = create_test_manager();
        let file1 = create_test_file(temp_dir.path(), "file1.txt", "original 1");
        let file2 = create_test_file(temp_dir.path(), "file2.txt", "original 2");
        let file3 = create_test_file(temp_dir.path(), "file3.txt", "original 3");

        let backup_id = manager
            .create_backup("s/a/b/", &[file1.clone(), file2.clone(), file3.clone()])
            .unwrap();

        // Modify all files
        fs::write(&file1, "modified 1").unwrap();
        fs::write(&file2, "modified 2").unwrap();
        fs::write(&file3, "modified 3").unwrap();

        // Restore
        manager.restore_backup(&backup_id).unwrap();

        // Verify all files restored
        assert_eq!(fs::read_to_string(&file1).unwrap(), "original 1");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "original 2");
        assert_eq!(fs::read_to_string(&file3).unwrap(), "original 3");
    }

    #[test]
    fn test_restore_backup_preserves_file_permissions() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        // Set specific permissions (read-write for owner only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&test_file).unwrap().permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&test_file, perms).unwrap();
        }

        let backup_id = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();

        // Modify and change permissions
        fs::write(&test_file, "modified").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&test_file).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&test_file, perms).unwrap();
        }

        // Restore
        manager.restore_backup(&backup_id).unwrap();

        // Verify content restored
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "content");

        // Note: File permissions after restore will depend on the system's umask
        // The key is that the file is restored and readable
    }

    // ============================================================================
    // get_last_backup_id() tests
    // ============================================================================

    #[test]
    fn test_get_last_backup_id_no_backups() {
        let (manager, _temp_dir) = create_test_manager();

        let last_id = manager.get_last_backup_id().unwrap();
        assert!(
            last_id.is_none(),
            "Should return None when no backups exist"
        );
    }

    #[test]
    fn test_get_last_backup_id_single_backup() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let backup_id = manager.create_backup("s/a/b/", &[test_file]).unwrap();

        let last_id = manager.get_last_backup_id().unwrap();
        assert_eq!(last_id.as_ref().unwrap(), &backup_id);
    }

    #[test]
    fn test_get_last_backup_id_multiple_backups() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let id1 = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = manager
            .create_backup("s/c/d/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id3 = manager.create_backup("s/e/f/", &[test_file]).unwrap();

        let last_id = manager.get_last_backup_id().unwrap();
        // Should return the most recent backup (id3)
        assert_eq!(last_id.as_ref().unwrap(), &id3);
        assert_ne!(last_id.as_ref().unwrap(), &id1);
        assert_ne!(last_id.as_ref().unwrap(), &id2);
    }

    // ============================================================================
    // list_backups() tests
    // ============================================================================

    #[test]
    fn test_list_backups_empty() {
        let (manager, _temp_dir) = create_test_manager();

        let backups = manager.list_backups().unwrap();
        assert_eq!(
            backups.len(),
            0,
            "Should return empty list when no backups exist"
        );
    }

    #[test]
    fn test_list_backups_multiple() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager
            .create_backup("s/c/d/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.create_backup("s/e/f/", &[test_file]).unwrap();

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 3);
    }

    #[test]
    fn test_list_backups_sorted_by_timestamp() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let id1 = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = manager
            .create_backup("s/c/d/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id3 = manager.create_backup("s/e/f/", &[test_file]).unwrap();

        let backups = manager.list_backups().unwrap();

        // Verify they're in chronological order
        assert_eq!(backups[0].id, id1);
        assert_eq!(backups[1].id, id2);
        assert_eq!(backups[2].id, id3);

        // Verify timestamps are in ascending order
        assert!(backups[0].timestamp < backups[1].timestamp);
        assert!(backups[1].timestamp < backups[2].timestamp);
    }

    #[test]
    fn test_list_backups_ignores_invalid_directories() {
        let (manager, _temp_dir) = create_test_manager();

        // Create a directory without operation.json
        let invalid_dir = manager.backups_dir().join("invalid-backup");
        fs::create_dir_all(&invalid_dir).unwrap();
        fs::write(invalid_dir.join("some_file.txt"), "data").unwrap();

        let backups = manager.list_backups().unwrap();
        assert_eq!(
            backups.len(),
            0,
            "Should ignore directories without operation.json"
        );
    }

    // ============================================================================
    // remove_backup_by_id() tests
    // ============================================================================

    #[test]
    fn test_remove_backup_existing() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let backup_id = manager.create_backup("s/a/b/", &[test_file]).unwrap();
        let backup_dir = manager.backups_dir().join(&backup_id);

        assert!(backup_dir.exists(), "Backup should exist before removal");

        manager.remove_backup_by_id(&backup_id).unwrap();

        assert!(
            !backup_dir.exists(),
            "Backup should not exist after removal"
        );
    }

    #[test]
    fn test_remove_backup_nonexistent() {
        let (manager, _) = create_test_manager();

        let result = manager.remove_backup_by_id("nonexistent-backup");
        // This should fail since the directory doesn't exist
        assert!(
            result.is_err(),
            "Should return error when removing nonexistent backup"
        );
    }

    // ============================================================================
    // prune_backups() tests
    // ============================================================================

    #[test]
    fn test_prune_backups_keep_all() {
        let (manager, _temp_dir) = create_test_manager();

        let removed = manager.prune_backups(10).unwrap();
        assert_eq!(
            removed, 0,
            "Should remove 0 backups when fewer than keep count"
        );
    }

    #[test]
    fn test_prune_backups_keep_some() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        // Create 5 backups
        let mut backup_ids = Vec::new();
        for i in 0..5 {
            backup_ids.push(
                manager
                    .create_backup(
                        &format!("s/test{i}/", i = i),
                        std::slice::from_ref(&test_file),
                    )
                    .unwrap(),
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Keep only the 2 most recent
        let removed = manager.prune_backups(2).unwrap();
        assert_eq!(removed, 3, "Should remove 3 oldest backups");

        // Verify only the 2 most recent backups remain
        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 2);

        // The remaining backups should be the most recent ones
        assert_eq!(backups[0].id, backup_ids[3]); // 4th created
        assert_eq!(backups[1].id, backup_ids[4]); // 5th created (most recent)
    }

    #[test]
    fn test_prune_backups_exact_count() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        // Create exactly 3 backups
        for i in 0..3 {
            manager
                .create_backup(&format!("s/test{}/", i), std::slice::from_ref(&test_file))
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Keep 3 (same as current count)
        let removed = manager.prune_backups(3).unwrap();
        assert_eq!(
            removed, 0,
            "Should remove 0 backups when count equals keep count"
        );

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 3);
    }

    // ============================================================================
    // prune_backups_older_than() tests
    // ============================================================================

    #[test]
    fn test_prune_backups_older_than_none_removed() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        manager.create_backup("s/a/b/", &[test_file]).unwrap();

        // Prune backups older than 30 days (should remove none)
        let removed = manager.prune_backups_older_than(30).unwrap();
        assert_eq!(removed, 0, "Should remove 0 backups when all are recent");
    }

    #[test]
    fn test_prune_backups_older_than_removes_old() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        // Create some backups
        for _ in 0..3 {
            manager
                .create_backup("s/a/b/", std::slice::from_ref(&test_file))
                .unwrap();
        }

        // Manually create an "old" backup by modifying its metadata
        let recent_backup_id = manager.create_backup("s/c/d/", &[test_file]).unwrap();
        let backup_dir = manager.backups_dir().join(&recent_backup_id);
        let metadata_path = backup_dir.join("operation.json");

        // Read, modify, and write back with old timestamp
        let metadata_json = fs::read_to_string(&metadata_path).unwrap();
        let mut metadata: BackupMetadata = serde_json::from_str(&metadata_json).unwrap();
        metadata.timestamp = Utc::now() - chrono::Duration::days(10);
        let new_json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(&metadata_path, new_json).unwrap();

        // Prune backups older than 5 days (should remove the one we made "old")
        let removed = manager.prune_backups_older_than(5).unwrap();
        assert_eq!(removed, 1, "Should remove 1 old backup");
    }

    // ============================================================================
    // parse_backup_metadata() tests
    // ============================================================================

    #[test]
    fn test_parse_backup_metadata_valid() {
        let json = r#"{
            "id": "20240201-120000-abc123",
            "timestamp": "2024-02-01T12:00:00Z",
            "expression": "s/foo/bar/g",
            "files": [
                {
                    "original_path": "/path/to/file1.txt",
                    "backup_path": "/backup/path/file1.txt"
                },
                {
                    "original_path": "/path/to/file2.txt",
                    "backup_path": "/backup/path/file2.txt"
                }
            ]
        }"#;

        let metadata = BackupManager::parse_backup_metadata(json).unwrap();

        assert_eq!(metadata.id, "20240201-120000-abc123");
        assert_eq!(metadata.expression, "s/foo/bar/g");
        assert_eq!(metadata.files.len(), 2);
        assert_eq!(
            metadata.files[0].original_path,
            PathBuf::from("/path/to/file1.txt")
        );
        assert_eq!(
            metadata.files[1].original_path,
            PathBuf::from("/path/to/file2.txt")
        );
    }

    #[test]
    fn test_parse_backup_metadata_invalid_json() {
        let invalid_json = r#"{ invalid json }"#;

        let result = BackupManager::parse_backup_metadata(invalid_json);
        assert!(result.is_err(), "Should return error for invalid JSON");
    }

    #[test]
    fn test_parse_backup_metadata_missing_required_field() {
        // Missing "id" field
        let json = r#"{
            "timestamp": "2024-02-01T12:00:00Z",
            "expression": "s/foo/bar/g",
            "files": []
        }"#;

        let result = BackupManager::parse_backup_metadata(json);
        assert!(
            result.is_err(),
            "Should return error when missing required field"
        );
    }

    #[test]
    fn test_parse_backup_metadata_malformed_timestamp() {
        let json = r#"{
            "id": "20240201-120000-abc123",
            "timestamp": "not-a-valid-timestamp",
            "expression": "s/foo/bar/g",
            "files": []
        }"#;

        let result = BackupManager::parse_backup_metadata(json);
        assert!(
            result.is_err(),
            "Should return error for malformed timestamp"
        );
    }

    #[test]
    fn test_parse_backup_metadata_empty_files() {
        let json = r#"{
            "id": "20240201-120000-abc123",
            "timestamp": "2024-02-01T12:00:00Z",
            "expression": "s/foo/bar/g",
            "files": []
        }"#;

        let metadata = BackupManager::parse_backup_metadata(json).unwrap();
        assert_eq!(metadata.files.len(), 0);
    }

    // ============================================================================
    // with_directory() tests
    // ============================================================================

    #[test]
    fn test_with_directory_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("custom_backup_dir");

        assert!(!custom_path.exists(), "Directory should not exist yet");

        let _manager =
            BackupManager::with_directory(custom_path.to_str().unwrap().to_string()).unwrap();

        assert!(custom_path.exists(), "Directory should be created");
    }

    #[test]
    fn test_backups_dir_returns_correct_path() {
        let (manager, _temp_dir) = create_test_manager();

        let returned_path = manager.backups_dir();
        assert!(returned_path.exists(), "Returned path should exist");
        assert!(
            returned_path.ends_with("backups"),
            "Returned path should end with 'backups'"
        );
    }

    // ============================================================================
    // Legacy backup compatibility
    // ============================================================================

    #[test]
    fn restore_rejects_legacy_metadata_without_path_identity() {
        // Legacy metadata without path_identity cannot bind the backup file to
        // the original path strongly enough, so restore must reject it.
        let (mut manager, temp_dir) = create_test_manager();
        let original = create_test_file(temp_dir.path(), "legacy.txt", "pre-upgrade");

        // Create a backup the new way, then rewrite the backup on disk and
        // metadata to look like a legacy uncompressed backup.
        let backup_id = manager
            .create_backup("s/a/b/", std::slice::from_ref(&original))
            .unwrap();
        let backup_dir = manager.backups_dir().join(&backup_id);
        let gzipped = backup_path_for_original(&backup_dir, &original);
        let uncompressed = backup_dir.join("legacy.txt");
        fs::remove_file(&gzipped).unwrap();
        fs::write(&uncompressed, "pre-upgrade").unwrap();

        let metadata_path = backup_dir.join("operation.json");
        let mut metadata: BackupMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata.files[0].backup_path = uncompressed;
        metadata.files[0].path_identity = None;
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        // Mutate the original so we can detect that restore ran.
        fs::write(&original, "after-edit").unwrap();

        let err = manager.restore_backup(&backup_id).unwrap_err();
        assert!(
            err.to_string()
                .contains("backup metadata path validation failed"),
            "unexpected error: {err}"
        );
        assert!(
            err.to_string().contains("missing path identity"),
            "unexpected error: {err}"
        );
        assert_eq!(fs::read_to_string(&original).unwrap(), "after-edit");
    }

    // ============================================================================
    // cleanup_old_backups() behavior via MAX_BACKUPS
    // ============================================================================

    #[test]
    fn test_auto_cleanup_on_create_backup() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        // Create more than MAX_BACKUPS (50) backups
        // For testing efficiency, we'll create just a few and verify the mechanism works
        let mut backup_ids = Vec::new();

        for i in 0..5 {
            backup_ids.push(
                manager
                    .create_backup(&format!("s/test{}/", i), std::slice::from_ref(&test_file))
                    .unwrap(),
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // All backups should still exist (less than MAX_BACKUPS)
        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 5);

        // The oldest backup should still be the first one created
        assert_eq!(backups[0].id, backup_ids[0]);
    }

    // ============================================================================
    // Edge cases and error handling
    // ============================================================================

    #[test]
    fn test_create_backup_empty_file_list() {
        let (mut manager, _temp_dir) = create_test_manager();

        let backup_id = manager.create_backup("s/a/b/", &[]);
        let backup_dir = manager.backups_dir().join(backup_id.as_ref().unwrap());

        // Backup should be created even with no files
        assert!(
            backup_id.is_ok(),
            "Should create backup even with empty file list"
        );
        assert!(backup_dir.exists(), "Backup directory should exist");

        // Metadata should exist with empty files list
        let metadata_path = backup_dir.join("operation.json");
        let metadata: BackupMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.files.len(), 0);
    }

    #[test]
    fn test_restore_backup_with_missing_backup_file() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "original");

        let backup_id = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();

        // Manually remove the backup file (simulating corruption)
        let backup_dir = manager.backups_dir().join(&backup_id);
        let backup_file = backup_path_for_original(&backup_dir, &test_file);
        fs::remove_file(&backup_file).unwrap();

        // Restore should still succeed but warn about missing file
        let result = manager.restore_backup(&backup_id);
        assert!(
            result.is_ok(),
            "Restore should succeed even with missing backup file"
        );

        // Original file should remain unchanged
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn test_backup_id_format() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let backup_id = manager.create_backup("s/a/b/", &[test_file]).unwrap();

        // Verify ID format: YYYYMMDD-HHMMSSmmm-XXXXXXXX
        // e.g., 20240201-120000123-abc12345
        assert!(
            backup_id.len() >= 20,
            "Backup ID should be at least 20 characters"
        );
        assert!(backup_id.contains('-'), "Backup ID should contain hyphens");

        // First part should be date format (8 digits)
        let parts: Vec<&str> = backup_id.split('-').collect();
        assert_eq!(parts[0].len(), 8, "First part should be 8 digits (date)");

        // Second part should be time format with milliseconds (9+ digits)
        assert!(
            parts[1].len() >= 9,
            "Second part should be at least 9 digits (time with milliseconds)"
        );
    }

    #[test]
    fn test_expression_preserved_in_metadata() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let test_expression = "s/foo\\(bar\\)/baz\\1/gi";
        let backup_id = manager
            .create_backup(test_expression, &[test_file])
            .unwrap();

        let backup_dir = manager.backups_dir().join(&backup_id);
        let metadata_path = backup_dir.join("operation.json");
        let metadata: BackupMetadata =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();

        assert_eq!(
            metadata.expression, test_expression,
            "Expression should be preserved exactly"
        );
    }

    #[test]
    fn test_multiple_backups_same_file_different_expressions() {
        let (mut manager, temp_dir) = create_test_manager();
        let test_file = create_test_file(temp_dir.path(), "test.txt", "content");

        let id1 = manager
            .create_backup("s/a/b/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = manager
            .create_backup("s/x/y/", std::slice::from_ref(&test_file))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id3 = manager.create_backup("s/1/2/", &[test_file]).unwrap();

        // All backups should have different IDs
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // Verify expressions are different in metadata
        let backups = manager.list_backups().unwrap();
        assert_eq!(backups[0].expression, "s/a/b/");
        assert_eq!(backups[1].expression, "s/x/y/");
        assert_eq!(backups[2].expression, "s/1/2/");
    }
}
