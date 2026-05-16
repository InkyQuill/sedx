//! Tests that the -E, -B, and default (PCRE) regex-flavor flags wire through
//! to the appropriate converter and produce the right behavior end-to-end.

mod common;

use common::sedx;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn default_is_pcre_dollar_backrefs() {
    // In PCRE mode, backreferences in the replacement use $1/$2.
    sedx()
        .arg(r"s/(foo)(bar)/$2$1/")
        .write_stdin("foobar\n")
        .assert()
        .success()
        .stdout("barfoo\n");
}

#[test]
fn pcre_specific_substitution_warns_when_configured() {
    let home = TempDir::new().unwrap();
    let config_dir = home.path().join(".sedx");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        r#"
[compatibility]
show_warnings = true
"#,
    )
    .unwrap();

    common::sedx_isolated(home.path())
        .arg(r"s/\d+/NUM/")
        .write_stdin("123\n")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Warning: PCRE-specific regex syntax",
        ));
}

#[test]
fn pcre_specific_substitution_warning_respects_config() {
    let home = TempDir::new().unwrap();
    let config_dir = home.path().join(".sedx");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        r#"
[compatibility]
show_warnings = false
"#,
    )
    .unwrap();

    common::sedx_isolated(home.path())
        .arg(r"s/\d+/NUM/")
        .write_stdin("123\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn ere_flag_accepts_backslash_backrefs_in_replacement() {
    // In ERE mode the parser converts \1/\2 in the replacement to PCRE $1/$2
    // internally, so the script writes GNU-sed-style backrefs.
    sedx()
        .args(["-E", r"s/(foo)(bar)/\2\1/"])
        .write_stdin("foobar\n")
        .assert()
        .success()
        .stdout("barfoo\n");
}

#[test]
fn bre_flag_requires_escaped_groups_and_quantifiers() {
    // In BRE mode, groups are \(...\) and +, ?, | are literal unless escaped.
    sedx()
        .args(["-B", r"s/\(foo\)\(bar\)/\2\1/"])
        .write_stdin("foobar\n")
        .assert()
        .success()
        .stdout("barfoo\n");
}

#[test]
fn escaped_paren_is_group_in_bre_and_literal_in_pcre() {
    // Proves -B is load-bearing with a clean differentiator:
    //   In BRE, `\(foo\)` is a group matching "foo".
    //   In PCRE, `\(foo\)` is a literal match for the string "(foo)".
    // Same input "foo" produces different results under each flavor.

    // BRE: matches and substitutes.
    sedx()
        .args(["-B", r"s/\(foo\)/X/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("X\n");

    // PCRE (default): no match; input is unchanged.
    sedx()
        .arg(r"s/\(foo\)/X/")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("foo\n");
}

#[test]
fn substitution_zero_occurrence_replaces_all_matches() {
    sedx()
        .arg("s/foo/bar/0")
        .write_stdin("foo foo\n")
        .assert()
        .success()
        .stdout("bar bar\n");
}

#[test]
fn bre_replacement_newline_produces_actual_newline() {
    sedx()
        .args(["-B", r"s/foo/bar\nbaz/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\nbaz\n");
}

#[test]
fn ere_replacement_newline_produces_actual_newline() {
    sedx()
        .args(["-E", r"s/foo/bar\nbaz/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\nbaz\n");
}

#[test]
fn ere_escaped_replacement_newline_stays_literal() {
    sedx()
        .args(["-E", r"s/foo/bar\\nbaz/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\\nbaz\n");
}

#[test]
fn ere_nth_substitution_expands_backrefs() {
    sedx()
        .args(["-E", r"s/(foo)(bar)/\2\1/2"])
        .write_stdin("foobar foobar\n")
        .assert()
        .success()
        .stdout("foobar barfoo\n");
}

#[test]
fn ere_nth_substitution_expands_whole_match() {
    sedx()
        .args(["-E", r"s/foobar/[\&]/2"])
        .write_stdin("foobar foobar\n")
        .assert()
        .success()
        .stdout("foobar [foobar]\n");
}
