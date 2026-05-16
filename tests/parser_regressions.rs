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
fn pattern_address_with_uppercase_t_before_test_false_does_not_panic() {
    common::sedx()
        .arg("/TEST/T done; :done")
        .write_stdin("TEST\n")
        .assert()
        .success();
}

#[test]
fn custom_delimiter_address_prints_matching_line_under_quiet_mode() {
    common::sedx()
        .args(["-n", r"\#alpha#p"])
        .write_stdin("alpha\nbeta\n")
        .assert()
        .success()
        .stdout("alpha\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn custom_delimiter_address_can_gate_branch_command() {
    common::sedx()
        .arg(r"\#alpha#b done; :done")
        .write_stdin("alpha\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
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
