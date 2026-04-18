//! Tests for stdin→stdout pipeline behavior, exit codes, and -e composition.

mod common;

use common::sedx;
use predicates::prelude::*;

#[test]
fn stdin_to_stdout_roundtrip() {
    sedx()
        .arg("s/foo/bar/")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\n");
}

#[test]
fn stdin_mode_exits_zero_on_success() {
    sedx()
        .arg("s/./X/")
        .write_stdin("a\n")
        .assert()
        .code(0);
}

#[test]
fn stdin_mode_exits_nonzero_on_parse_error() {
    sedx()
        .arg("s/unclosed") // no closing delimiter
        .write_stdin("a\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse expression"));
}

#[test]
fn multiple_e_flags_compose_left_to_right() {
    // Two -e expressions: the second runs after the first.
    sedx()
        .args(["-e", "s/foo/X/", "-e", "s/X/Y/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("Y\n");
}

#[test]
fn stdin_output_is_backup_and_diff_free() {
    // In pipeline mode sedx must emit only the transformed text — no
    // "Backup created:", no "Total:", no "Rollback with:" lines.
    let output = sedx()
        .arg("s/foo/bar/")
        .write_stdin("foo\nbaz\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert_eq!(text, "bar\nbaz\n");
}
