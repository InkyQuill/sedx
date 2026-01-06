use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const MAX_BACKUPS: usize = 50;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub expression: String,
    pub files: Vec<FileBackup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileBackup {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
}

pub struct BackupManager {
    backups_dir: PathBuf,
}

impl BackupManager {
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let backups_dir = home_dir.join(".sedx").join("backups");

        // Create backups directory if it doesn't exist
        fs::create_dir_all(&backups_dir)
            .with_context(|| format!("Failed to create backups directory: {}", backups_dir.display()))?;

        Ok(Self { backups_dir })
    }

    pub fn create_backup(&mut self, expression: &str, files: &[PathBuf]) -> Result<String> {
        // Generate unique backup ID
        let id = format!("{}-{}", Utc::now().format("%Y%m%d-%H%M%S"), Uuid::new_v4().to_string().split_at(8).0);
        let backup_dir = self.backups_dir.join(&id);

        fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;

        let mut file_backups = Vec::new();

        for file_path in files {
            if !file_path.exists() {
                continue;
            }

            let file_name = file_path.file_name()
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
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize metadata")?;

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

        let metadata: BackupMetadata = serde_json::from_str(&metadata_json)
            .context("Failed to parse metadata")?;

        for file_backup in &metadata.files {
            if !file_backup.backup_path.exists() {
                eprintln!("Warning: Backup file missing: {}", file_backup.backup_path.display());
                continue;
            }

            fs::copy(&file_backup.backup_path, &file_backup.original_path)
                .with_context(|| format!("Failed to restore file: {}", file_backup.original_path.display()))?;

            println!("Restored: {}", file_backup.original_path.display());
        }

        // Remove backup after successful restore
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("Failed to remove backup directory: {}", backup_dir.display()))?;

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

        for entry in fs::read_dir(&self.backups_dir)
            .with_context(|| format!("Failed to read backups directory: {}", self.backups_dir.display()))?
        {
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

        Ok(backups)
    }

    fn cleanup_old_backups(&self) -> Result<()> {
        let mut backups = self.list_backups()?;
        backups.sort_by_key(|b| b.timestamp);

        if backups.len() > MAX_BACKUPS {
            for backup in backups.iter().take(backups.len() - MAX_BACKUPS) {
                let backup_dir = self.backups_dir.join(&backup.id);
                fs::remove_dir_all(&backup_dir)
                    .with_context(|| format!("Failed to remove old backup: {}", backup_dir.display()))?;
            }
        }

        Ok(())
    }
}
