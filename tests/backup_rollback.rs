//! Tests for backup creation, rollback roundtrip, history listing, --no-backup.

mod common;

use common::{read_file, sedx_isolated, write_file};
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn backups_dir(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".sedx").join("backups")
}

fn newest_backup_id(home: &std::path::Path) -> String {
    let entries = std::fs::read_dir(backups_dir(home)).expect("backups dir exists");
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .max()
        .expect("at least one backup")
}

#[test]
fn edit_creates_backup_with_metadata() {
    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "foo\n");

    sedx_isolated(home.path())
        .args(["s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success();

    let id = newest_backup_id(home.path());
    let metadata_path = backups_dir(home.path()).join(&id).join("operation.json");
    let metadata = read_file(&metadata_path);

    assert!(
        metadata.contains("\"expression\""),
        "metadata: {}",
        metadata
    );
    assert!(metadata.contains("s/foo/bar/"), "metadata: {}", metadata);
    assert!(metadata.contains(&id), "metadata: {}", metadata);
}

#[test]
fn rollback_restores_exact_original_bytes() {
    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "foo bar baz\n");
    let original = read_file(&file);

    sedx_isolated(home.path())
        .args(["s/foo/X/", file.to_str().unwrap()])
        .assert()
        .success();

    assert_ne!(read_file(&file), original); // confirm edit happened
    let id = newest_backup_id(home.path());

    sedx_isolated(home.path())
        .args(["rollback", &id])
        .assert()
        .success();

    assert_eq!(read_file(&file), original);
}

#[test]
fn history_lists_recent_backups() {
    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "aaa\n");

    sedx_isolated(home.path())
        .args(["s/a/A/", file.to_str().unwrap()])
        .assert()
        .success();

    sedx_isolated(home.path())
        .args(["history"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Operation History"))
        .stdout(predicate::str::contains("s/a/A/"));
}

#[test]
fn no_backup_flag_skips_backup_creation() {
    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "foo\n");

    sedx_isolated(home.path())
        .args([
            "--no-backup",
            "--force",
            "s/foo/bar/",
            file.to_str().unwrap(),
        ])
        .assert()
        .success();

    // File was modified.
    assert_eq!(read_file(&file), "bar\n");
    // No ~/.sedx/backups/ directory created.
    assert!(
        !backups_dir(home.path()).exists()
            || backups_dir(home.path())
                .read_dir()
                .map(|mut r| r.next().is_none())
                .unwrap_or(true),
        "backup directory should be empty or absent"
    );
}

#[test]
fn backup_prune_keep_days_removes_old_backups() {
    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "aaa\n");

    sedx_isolated(home.path())
        .args(["s/a/A/", file.to_str().unwrap()])
        .assert()
        .success();
    let old_id = newest_backup_id(home.path());

    sedx_isolated(home.path())
        .args(["s/A/B/", file.to_str().unwrap()])
        .assert()
        .success();
    let recent_id = newest_backup_id(home.path());

    let old_metadata_path = backups_dir(home.path())
        .join(&old_id)
        .join("operation.json");
    let mut metadata: Value = serde_json::from_str(&read_file(&old_metadata_path)).unwrap();
    metadata["timestamp"] =
        Value::String((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339());
    std::fs::write(
        &old_metadata_path,
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    sedx_isolated(home.path())
        .args(["backup", "prune", "--keep-days=7", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pruned 1 backups."));

    assert!(!backups_dir(home.path()).join(old_id).exists());
    assert!(backups_dir(home.path()).join(recent_id).exists());
}
