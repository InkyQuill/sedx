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
fn custom_delimiter_address_with_comma_prints_matching_line_under_quiet_mode() {
    common::sedx()
        .args(["-n", r"\#a,b#p"])
        .write_stdin("a,b\nother\n")
        .assert()
        .success()
        .stdout("a,b\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn custom_delimiter_range_with_comma_in_start_pattern_prints_range() {
    common::sedx()
        .args(["-n", r"\#a,b#,\#c#p"])
        .write_stdin("before\na,b\nc\nafter\n")
        .assert()
        .success()
        .stdout("a,b\nc\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn slash_delimited_pattern_range_can_gate_slash_delimited_substitution() {
    common::sedx()
        .arg(r"/A/,/B/s/^/x:/")
        .write_stdin("before\nA\nmid\nB\nafter\n")
        .assert()
        .success()
        .stdout("before\nx:A\nx:mid\nx:B\nafter\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn grouped_custom_delimiter_range_with_comma_in_start_pattern_prints_range() {
    common::sedx()
        .args(["-n", r"\#a,b#,\#c#{p}"])
        .write_stdin("before\na,b\nc\nafter\n")
        .assert()
        .success()
        .stdout("a,b\nc\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn grouped_custom_delimiter_range_with_brace_in_start_pattern_prints_range() {
    common::sedx()
        .args(["-n", r"\#a\{b#,\#c#{p}"])
        .write_stdin("before\na{b\nc\nafter\n")
        .assert()
        .success()
        .stdout("a{b\nc\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn top_level_splitter_ignores_escaped_brace_in_custom_delimiter_address() {
    common::sedx()
        .args(["-n", r"\#a\{b#,\#c#{p};p"])
        .write_stdin("before\na{b\nc\nafter\n")
        .assert()
        .success()
        .stdout("before\na{b\na{b\nc\nc\nafter\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn group_parser_rejects_trailing_command_without_semicolon_separator() {
    common::sedx()
        .arg("{p}p")
        .write_stdin("line\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unexpected trailing content after group command: p",
        ));
}

#[test]
fn group_parser_ignores_literal_closing_brace_in_custom_delimiter_address() {
    common::sedx()
        .args(["-n", r"{\#a}b#p};p"])
        .write_stdin("x\na}b\n")
        .assert()
        .success()
        .stdout("x\na}b\na}b\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn group_parser_ignores_literal_closing_brace_in_substitution_replacement() {
    common::sedx()
        .arg(r"{s/foo/}/;p}")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("}\n}\n")
        .stderr(predicate::str::is_empty());
}

#[test]
fn malformed_substitution_inside_group_reports_substitution_error() {
    common::sedx()
        .arg(r"{s/foo/bar};p}")
        .write_stdin("foo\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "missing closing delimiter: missing third '/' delimiter after replacement",
        ));
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
