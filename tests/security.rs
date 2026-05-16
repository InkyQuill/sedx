mod common;

use common::{read_file, sedx_isolated, write_file};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn write_file_command_rejects_absolute_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");
    let outside = dir.join("outside.txt");

    sedx_isolated(dir)
        .args([
            "--no-backup",
            "--force",
            &format!("w {}", outside.display()),
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"))
        .stderr(predicate::str::contains("absolute paths are not allowed"));

    assert!(!outside.exists());
}

#[test]
fn write_file_command_rejects_parent_traversal() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let work = dir.join("work");
    std::fs::create_dir(&work).unwrap();
    let input = write_file(&work, "input.txt", "alpha\n");

    sedx_isolated(dir)
        .current_dir(&work)
        .args([
            "--no-backup",
            "--force",
            "w ../outside.txt",
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"))
        .stderr(predicate::str::contains("parent traversal is not allowed"));

    assert!(!dir.join("outside.txt").exists());
}

#[test]
fn read_file_command_rejects_absolute_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");
    let secret = write_file(dir, "secret.txt", "secret\n");

    sedx_isolated(dir)
        .args([
            "--dry-run",
            &format!("r {}", secret.display()),
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"));
}

#[test]
fn forced_streaming_rejects_read_file_command() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\nbravo\n");
    write_file(dir, "extra.txt", "extra\n");

    sedx_isolated(dir)
        .current_dir(dir)
        .args(["--streaming", "2r extra.txt", input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not supported in forced streaming mode",
        ));
}

#[test]
fn forced_streaming_rejects_grouped_read_file_command() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\nbravo\n");
    write_file(dir, "extra.txt", "extra\n");

    sedx_isolated(dir)
        .current_dir(dir)
        .args(["--streaming", "{2r extra.txt}", input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not supported in forced streaming mode",
        ));
}

#[test]
fn backup_restore_rejects_tampered_original_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "foo\n");
    let target = write_file(dir, "target.txt", "do not overwrite\n");

    sedx_isolated(dir)
        .args(["s/foo/bar/", input.to_str().unwrap()])
        .assert()
        .success();

    let backups_dir = dir.join(".sedx").join("backups");
    let backup_id = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.path().is_dir())
        .unwrap()
        .file_name()
        .to_string_lossy()
        .into_owned();

    let metadata_path = backups_dir.join(&backup_id).join("operation.json");
    let mut metadata: serde_json::Value = serde_json::from_str(&read_file(&metadata_path)).unwrap();
    metadata["files"][0]["original_path"] =
        serde_json::Value::String(target.to_string_lossy().into_owned());
    std::fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    sedx_isolated(dir)
        .args(["rollback", &backup_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "backup metadata path validation failed",
        ));

    assert_eq!(read_file(&target), "do not overwrite\n");
}

#[cfg(unix)]
#[test]
fn edit_rejects_symlink_target() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "secret\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(dir)
        .args([
            "--no-backup",
            "--force",
            "s/secret/public/",
            link.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink targets are not allowed"));

    assert_eq!(read_file(&target), "secret\n");
}

#[cfg(unix)]
#[test]
fn read_file_command_rejects_symlink_operand() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");
    let target = write_file(dir, "target.txt", "secret\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(dir)
        .current_dir(dir)
        .args(["--dry-run", "r link.txt", input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink targets are not allowed"));
}

#[cfg(unix)]
#[test]
fn write_file_command_rejects_symlink_directory_operand() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let outside = TempDir::new().unwrap();
    let input = write_file(dir, "input.txt", "alpha\n");
    let linkdir = dir.join("linkdir");
    let outside_pwned = outside.path().join("pwned.txt");
    symlink(outside.path(), &linkdir).unwrap();

    sedx_isolated(dir)
        .current_dir(dir)
        .args([
            "--no-backup",
            "--force",
            "w linkdir/pwned.txt",
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink targets are not allowed"));

    assert!(!outside_pwned.exists());
}
