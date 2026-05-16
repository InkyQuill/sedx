mod common;

use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn pattern_address_with_b_before_branch_does_not_panic() {
    common::sedx()
        .arg("/bar/b skip; :skip")
        .write_stdin("AAA\nbar\nCCC\n")
        .assert()
        .success();
}

#[test]
fn pattern_address_with_t_before_test_branch_does_not_panic() {
    common::sedx()
        .arg("/test/t done")
        .write_stdin("test\n")
        .assert()
        .success();
}

#[test]
fn custom_delimiter_address_does_not_confuse_write_command_detection() {
    let home = TempDir::new().unwrap();

    common::sedx()
        .current_dir(home.path())
        .arg(r"\#alpha#w out.txt")
        .write_stdin("alpha\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}
