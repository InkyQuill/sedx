//! Tests for atomic write semantics: Unix mode preservation and symlink follow.
//! Covers both the in-memory (<100MB) and streaming (≥100MB) write paths.

mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn editing_via_symlink_writes_to_target_not_link() {
    // Regression: editing `link` that points at `target` must modify `target`
    // in place and leave the symlink intact. Previously the atomic rename
    // replaced the symlink with a regular file.
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "old\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(home.path())
        .args(["s/old/new/", link.to_str().unwrap()])
        .assert()
        .success();

    // Link still a symlink, target updated.
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(read_file(&target), "new\n");
}
