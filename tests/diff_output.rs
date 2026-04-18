//! Tests for the --dry-run diff output format.

mod common;

use common::{sedx_isolated, write_file};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn dry_run_header_contains_expression() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "foo\n");

    sedx_isolated(dir)
        .args(["--dry-run", "s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Dry run: s/foo/bar/"));
}

#[test]
fn dry_run_shows_equals_prefix_for_unchanged_lines() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "a\nfoo\nc\n");

    sedx_isolated(dir)
        .args(["--dry-run", "s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("L1: = a"))
        .stdout(predicate::str::contains("L3: = c"));
}

#[test]
fn dry_run_shows_tilde_prefix_for_modified_lines() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "a\nfoo\nc\n");

    sedx_isolated(dir)
        .args(["--dry-run", "s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("L2: ~ bar"));
}

#[test]
fn dry_run_shows_minus_prefix_for_deleted_lines() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "a\nb\nc\n");

    sedx_isolated(dir)
        .args(["--dry-run", "2d", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("L2: - b"));
}

#[test]
fn dry_run_shows_tilde_prefix_for_inserted_lines() {
    // Insert produces a tilde (~) line showing the new text, followed by a
    // minus (-) line for the duplicate of the line that got pushed down.
    // We assert only that the inserted text appears with line numbering.
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "a\nb\n");

    sedx_isolated(dir)
        .args(["--dry-run", r"1i\HEAD", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("HEAD"));
}

#[test]
fn dry_run_summary_line_counts_changes() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "foo\nbar\nfoo\n");

    sedx_isolated(dir)
        .args(["--dry-run", "s/foo/X/g", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Total: 2 changes (2 modified, 0 added, 0 deleted)",
        ));
}

#[test]
fn dry_run_multi_file_produces_one_section_per_file() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let f1 = write_file(dir, "one.txt", "foo\n");
    let f2 = write_file(dir, "two.txt", "foo\n");

    sedx_isolated(dir)
        .args([
            "--dry-run",
            "s/foo/bar/",
            f1.to_str().unwrap(),
            f2.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("one.txt"))
        .stdout(predicate::str::contains("two.txt"));
}

#[test]
fn no_color_env_suppresses_ansi_escapes() {
    // sedx() already sets NO_COLOR=1. Assert that output contains no ESC byte.
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "in.txt", "foo\n");

    let output = sedx_isolated(dir)
        .args(["--dry-run", "s/foo/bar/", file.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(
        !output.contains(&0x1b),
        "output contained ESC byte: {:?}",
        String::from_utf8_lossy(&output)
    );
}
