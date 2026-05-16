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
fn ere_bare_ampersand_expands_whole_match() {
    sedx()
        .args(["-E", r"s/foo/[&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[foo]\n");
}

#[test]
fn ere_zero_backreference_expands_whole_match() {
    sedx()
        .args(["-E", r"s/(foo)/[\0]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[foo]\n");
}

#[test]
fn ere_escaped_ampersand_is_literal() {
    sedx()
        .args(["-E", r"s/foo/[\&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[&]\n");
}

#[test]
fn ere_replacement_dollar_is_literal_before_ampersand() {
    sedx()
        .args(["-E", r"s/foo/[$&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[$foo]\n");
}

#[test]
fn ere_replacement_double_dollar_is_literal() {
    sedx()
        .args(["-E", r"s/foo/[$$]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[$$]\n");
}

#[test]
fn escaped_delimiter_in_pattern_is_literal() {
    sedx()
        .arg(r"s/a\/b/X/")
        .write_stdin("a/b\n")
        .assert()
        .success()
        .stdout("X\n");
}

#[test]
fn escaped_delimiter_in_replacement_is_literal() {
    sedx()
        .arg(r"s/foo/a\/b/")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("a/b\n");
}

#[test]
fn escaped_pipe_delimiter_in_pattern_is_literal() {
    sedx()
        .arg(r"s|a\|b|X|")
        .write_stdin("a|b\na\nb\n")
        .assert()
        .success()
        .stdout("X\na\nb\n");
}

#[test]
fn bre_escaped_pipe_delimiter_in_pattern_is_literal() {
    sedx()
        .args(["-B", r"s|a\|b|X|"])
        .write_stdin("a|b\na\nb\n")
        .assert()
        .success()
        .stdout("X\na\nb\n");
}

#[test]
fn escaped_pipe_delimiter_in_replacement_is_literal() {
    sedx()
        .arg(r"s|foo|a\|b|")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("a|b\n");
}

#[test]
fn semicolon_inside_substitution_pattern_does_not_split_command() {
    sedx()
        .arg("s/a;b/X/")
        .write_stdin("a;b\n")
        .assert()
        .success()
        .stdout("X\n");
}

#[test]
fn semicolon_inside_substitution_replacement_does_not_split_command() {
    sedx()
        .arg("s/foo/a;b/")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("a;b\n");
}

#[test]
fn ere_replacement_double_dollar_before_ampersand_is_literal_dollars_then_match() {
    sedx()
        .args(["-E", r"s/foo/[$$&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[$$foo]\n");
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
fn bre_bare_ampersand_expands_whole_match() {
    sedx()
        .args(["-B", r"s/foo/[&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[foo]\n");
}

#[test]
fn bre_zero_backreference_expands_whole_match() {
    sedx()
        .args(["-B", r"s/\(foo\)/[\0]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[foo]\n");
}

#[test]
fn bre_replacement_dollar_is_literal_before_ampersand() {
    sedx()
        .args(["-B", r"s/foo/[$&]/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[$foo]\n");
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
fn substitution_multi_digit_nth_match_flag() {
    sedx()
        .arg("s/a/X/10")
        .write_stdin("a a a a a a a a a a a\n")
        .assert()
        .success()
        .stdout("a a a a a a a a a X a\n");
}

#[test]
fn substitution_nth_plus_global_replaces_from_nth_match_onward() {
    sedx()
        .arg("s/a/X/3g")
        .write_stdin("a a a a\n")
        .assert()
        .success()
        .stdout("a a X X\n");
}

#[test]
fn substitution_nth_plus_global_replaces_zero_width_match_at_start() {
    sedx()
        .arg("s/^/X/1g")
        .write_stdin("abc\n")
        .assert()
        .success()
        .stdout("Xabc\n");
}

#[test]
fn direct_dollar_ampersand_expands_whole_match() {
    sedx()
        .arg(r"s/foobar/[$&]/")
        .write_stdin("foobar\n")
        .assert()
        .success()
        .stdout("[foobar]\n");
}

#[test]
fn direct_dollar_ampersand_expands_whole_match_for_nth_substitution() {
    sedx()
        .arg(r"s/(foo)(bar)/[$&]/2")
        .write_stdin("foobar foobar\n")
        .assert()
        .success()
        .stdout("foobar [foobar]\n");
}

#[test]
fn escaped_dollar_preserves_literal_dollar_ampersand() {
    sedx()
        .arg(r"s/foo/[$$&]/")
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("[$&]\n");
}

#[test]
fn escaped_dollar_preserves_literal_dollar_ampersand_for_nth_substitution() {
    sedx()
        .arg(r"s/foo/[$$&]/2")
        .write_stdin("foo foo\n")
        .assert()
        .success()
        .stdout("foo [$&]\n");
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
        .args(["-E", r"s/foobar/[&]/2"])
        .write_stdin("foobar foobar\n")
        .assert()
        .success()
        .stdout("foobar [foobar]\n");
}
