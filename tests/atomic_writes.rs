//! Tests for atomic write semantics: Unix mode preservation and symlink rejection.
//! Covers both the in-memory (<100MB) and streaming (≥100MB) write paths.
//!
//! Every test here exercises a Unix-only invariant (mode bits, symlinks), so
//! the whole file is gated — on Windows the test binary has no tests.

#![cfg(unix)]

mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn editing_via_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "old\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(home.path())
        .args(["s/old/new/", link.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("symlink targets are not allowed"));

    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(read_file(&target), "old\n");
}

#[cfg(unix)]
#[test]
fn in_memory_edit_preserves_unix_mode() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "foo\n");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

    sedx_isolated(home.path())
        .args(["s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success();

    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "expected mode 0o755, got {:o}", mode);
    assert_eq!(read_file(&file), "bar\n");
}

#[cfg(unix)]
#[test]
fn streaming_edit_preserves_unix_mode() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "foo\n");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

    sedx_isolated(home.path())
        .args(["--streaming", "s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success();

    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "expected mode 0o755, got {:o}", mode);
    assert_eq!(read_file(&file), "bar\n");
}
