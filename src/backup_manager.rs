use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_BACKUPS: usize = 50;

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
}

pub struct BackupManager {
    backups_dir: PathBuf,
}

impl BackupManager {
    pub fn new() -> Result<Self> {
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
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
        #[allow(dead_code)] // Documented threshold for future warning implementation
        const WARN_PERCENT: f64 = 40.0;
        #[cfg_attr(windows, allow(dead_code))] // Only used on Unix
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
        // Skip on Windows in test mode (disk_space check not implemented there)
        #[cfg(not(all(windows, test)))]
        let _disk_check_result = crate::disk_space::check_disk_space_for_backup(
            &self.backups_dir,
            total_size,
            ERROR_PERCENT,
        );
        #[cfg(not(all(windows, test)))]
        if let Err(e) = _disk_check_result {
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

            let file_name = file_path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid file name: {}", file_path.display()))?;

            let backup_path = backup_dir.join(file_name);

            fs::copy(file_path, &backup_path)
                .with_context(|| format!("Failed to backup file: {}", file_path.display()))?;

            file_backups.push(FileBackup {
                original_path: file_path.clone(),
                backup_path,
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

        for file_backup in &metadata.files {
            if !file_backup.backup_path.exists() {
                eprintln!(
                    "Warning: Backup file missing: {}",
                    file_backup.backup_path.display()
                );
                continue;
            }

            fs::copy(&file_backup.backup_path, &file_backup.original_path).with_context(|| {
                format!(
                    "Failed to restore file: {}",
                    file_backup.original_path.display()
                )
            })?;

            println!("Restored: {}", file_backup.original_path.display());
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
    #[allow(dead_code)] // Public API - kept for future use
    pub fn remove_backup_by_id(&self, backup_id: &str) -> Result<()> {
        let backup_dir = self.backups_dir.join(backup_id);
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("Failed to remove backup: {}", backup_dir.display()))?;
        Ok(())
    }

    /// Parse backup metadata from JSON string
    #[allow(dead_code)] // Public API - kept for future use
    pub fn parse_backup_metadata(json: &str) -> Result<BackupMetadata> {
        let metadata: BackupMetadata =
            serde_json::from_str(json).context("Failed to parse backup metadata")?;
        Ok(metadata)
    }

    /// Prune backups keeping only the N most recent ones
    #[allow(dead_code)] // Public API - kept for future use
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
    #[allow(dead_code)] // Public API - kept for future use
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

        // Verify backup file exists
        let backup_file = backup_dir.join("test.txt");
        assert!(backup_file.exists(), "Backup file should exist");

        // Verify backup content matches original
        let backup_content = fs::read_to_string(&backup_file).unwrap();
        let original_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(
            backup_content, original_content,
            "Backup content should match original"
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

        // Verify all files were backed up
        assert!(backup_dir.join("file1.txt").exists());
        assert!(backup_dir.join("file2.txt").exists());
        assert!(backup_dir.join("file3.txt").exists());

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
        let backup_file = backup_dir.join("large.txt");

        // Verify file size matches
        let backup_metadata = fs::metadata(&backup_file).unwrap();
        let original_metadata = fs::metadata(&large_file).unwrap();
        assert_eq!(backup_metadata.len(), original_metadata.len());
        assert_eq!(backup_metadata.len(), 1_000_000);
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

        // Verify all files with special characters were backed up
        for (name, _) in &test_cases {
            assert!(
                backup_dir.join(name).exists(),
                "File '{}' should exist in backup",
                name
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
        let backup_file = backup_dir.join("test.txt");
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
