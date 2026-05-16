//! One end-to-end smoke test per `Command` variant. Catches parser↔executor
//! wiring bugs that unit tests cannot reach.

mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[test]
fn pattern_address_insert_modifies_file() {
    // Regression: pattern-address i\/a\/c used to be routed to a streaming
    // path that silently dropped the command. Now routed to in-memory.
    let home = TempDir::new().unwrap();
    let input = home.path();
    let file = write_file(input, "in.txt", "alpha\nbravo\ncharlie\n");

    sedx_isolated(home.path())
        .args([r"/bravo/i\INSERTED", file.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(&file), "alpha\nINSERTED\nbravo\ncharlie\n");
}

#[test]
fn substitution_first_match_only_by_default() {
    common::sedx()
        .arg("s/foo/X/")
        .write_stdin("foo foo foo\n")
        .assert()
        .success()
        .stdout("X foo foo\n");
}

#[test]
fn substitution_global_flag_replaces_all() {
    common::sedx()
        .arg("s/foo/X/g")
        .write_stdin("foo foo foo\n")
        .assert()
        .success()
        .stdout("X X X\n");
}

#[test]
fn substitution_case_insensitive_flag() {
    common::sedx()
        .arg("s/foo/X/i")
        .write_stdin("FOO Foo foo\n")
        .assert()
        .success()
        .stdout("X Foo foo\n");
}

#[test]
fn substitution_nth_match_flag() {
    common::sedx()
        .arg("s/foo/X/2")
        .write_stdin("foo foo foo\n")
        .assert()
        .success()
        .stdout("foo X foo\n");
}

#[test]
fn delete_by_line_range() {
    common::sedx()
        .arg("2,3d")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("a\nd\n");
}

#[test]
fn print_with_quiet_emits_only_addressed_lines() {
    common::sedx()
        .args(["-n", "2,3p"])
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("b\nc\n");
}

#[test]
fn quit_prints_then_stops() {
    // `q` is a read-only command: it prints lines up to the quit point but
    // must NOT modify the file on disk.
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let original = "line 1\nline 2\nline 3\n";
    let file = common::write_file(home.path(), "in.txt", original);
    let output = common::sedx_isolated(home.path())
        .args(["2q", file.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    // The printed output must contain the lines seen up to the quit point.
    assert!(
        stdout.contains("line 1"),
        "stdout should contain 'line 1', got: {stdout:?}"
    );
    assert!(
        stdout.contains("line 2"),
        "stdout should contain 'line 2', got: {stdout:?}"
    );
    // File on disk must be completely unchanged.
    assert_eq!(
        common::read_file(&file),
        original,
        "q is read-only: file must not be modified"
    );
}

#[test]
fn quit_without_print_stops_without_emitting_current_line() {
    common::sedx()
        .arg("2Q")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn insert_before_line() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let file = common::write_file(home.path(), "in.txt", "a\nb\nc\n");
    common::sedx_isolated(home.path())
        .args([r"2i\INSERTED", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(common::read_file(&file), "a\nINSERTED\nb\nc\n");
}

#[test]
fn stdin_insert_before_line() {
    common::sedx()
        .arg(r"2i\INSERTED")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nINSERTED\nb\nc\n");
}

#[test]
fn append_after_line() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let file = common::write_file(home.path(), "in.txt", "a\nb\nc\n");
    common::sedx_isolated(home.path())
        .args([r"2a\APPENDED", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(common::read_file(&file), "a\nb\nAPPENDED\nc\n");
}

#[test]
fn stdin_append_after_line() {
    common::sedx()
        .arg(r"2a\APPENDED")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nb\nAPPENDED\nc\n");
}

#[test]
fn change_single_line() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let file = common::write_file(home.path(), "in.txt", "a\nb\nc\n");
    common::sedx_isolated(home.path())
        .args([r"2c\REPLACED", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(common::read_file(&file), "a\nREPLACED\nc\n");
}

#[test]
fn stdin_change_single_line() {
    common::sedx()
        .arg(r"2c\REPLACED")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nREPLACED\nc\n");
}

#[test]
fn change_range_collapses_to_single_line() {
    // 2,3c\TEXT replaces both lines with one TEXT line (GNU-sed compatible).
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let file = common::write_file(home.path(), "in.txt", "a\nb\nc\nd\n");
    common::sedx_isolated(home.path())
        .args([r"2,3c\REPLACED", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(common::read_file(&file), "a\nREPLACED\nd\n");
}

#[test]
fn stdin_change_range_collapses_to_single_line() {
    common::sedx()
        .arg(r"2,3c\REPLACED")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("a\nREPLACED\nd\n");
}

#[test]
fn group_runs_inner_commands_in_order() {
    common::sedx()
        .arg("{s/a/A/g; s/b/B/g}")
        .write_stdin("ab\nba\n")
        .assert()
        .success()
        .stdout("AB\nBA\n");
}

#[test]
fn hold_copy_and_get() {
    // h at line 1 → hold="a"; g at line 3 → pattern = "a".
    common::sedx()
        .arg("1h; 3g")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nb\na\n");
}

#[test]
fn hold_append_and_get() {
    // H at 1,2 builds up hold space; g at 3 copies it back into pattern space.
    // sedx prints the resulting pattern space without emitting the leading
    // empty line that H's implicit "\n" separator produces on an empty hold.
    common::sedx()
        .arg("1,2H; 3g")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nb\na\nb\n");
}

#[test]
fn get_append_appends_hold_to_pattern() {
    // h at 1 → hold="a"; G at 3 → pattern="c\na".
    common::sedx()
        .arg("1h; 3G")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nb\nc\na\n");
}

#[test]
fn exchange_swaps_pattern_and_hold() {
    // 1h → hold="a"; 2x → pattern="a", hold="b"; 3x → pattern="b", hold="c".
    common::sedx()
        .arg("1h; 2x; 3x")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\na\nb\n");
}

#[test]
fn next_advances_to_next_line() {
    // N appends next line to pattern space; s then operates across both.
    common::sedx()
        .arg(r"N; s/\n/ /")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("a b\nc d\n");
}

#[test]
fn print_first_line_of_pattern_space() {
    // Build 2-line pattern space with N, then P prints first line only.
    common::sedx()
        .arg("N; P; d")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("a\nc\n");
}

#[test]
fn delete_first_line_of_pattern_space_consumes_pairs() {
    // GNU sed: N at EOF must print pattern space, not drop it.
    // N;D on a 4-line file: pairs (a,b)→D keeps b, (b→c)… until pattern_space="d".
    // When N hits EOF on the last unpaired line, pattern space is flushed and script ends.
    common::sedx()
        .arg("N; D")
        .write_stdin("a\nb\nc\nd\n")
        .assert()
        .success()
        .stdout("d\n");
}

#[test]
fn n_at_eof_on_odd_input_prints_last_line() {
    // N; s/\n/ /  on 3 lines: a+N→"a\nb"→subst→"a b" (prints), then
    // pattern space = "c", N hits EOF → GNU sed flushes "c" and ends.
    common::sedx()
        .arg(r"N; s/\n/ /")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a b\nc\n");
}

#[test]
fn label_and_unconditional_branch() {
    // b skips to :end, so the second s/ is never applied to line 1.
    common::sedx()
        .arg(r":top; b end; s/a/A/; :end")
        .write_stdin("a\n")
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn test_branch_fires_after_substitution() {
    // t end branches when the preceding s/ succeeded. Line "foo" matches
    // and skips the second s/. Line "bar" doesn't match → both s/ run.
    common::sedx()
        .arg(r"s/foo/FOO/; t end; s/bar/BAR/; :end")
        .write_stdin("foo\nbar\n")
        .assert()
        .success()
        .stdout("FOO\nBAR\n");
}

#[test]
fn test_false_branch_fires_when_no_substitution() {
    common::sedx()
        .arg(r"s/zzz/XXX/; T end; s/.*/unreachable/; :end")
        .write_stdin("abc\n")
        .assert()
        .success()
        .stdout("abc\n");
}

#[test]
fn read_file_appends_contents_after_address() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let data = common::write_file(home.path(), "extra.txt", "X1\nX2\n");

    common::sedx()
        .arg(format!("2r {}", data.to_str().unwrap()))
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nb\nX1\nX2\nc\n");
}

#[test]
fn file_mode_read_file_appends_contents_after_address() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = common::write_file(dir, "in.txt", "a\nb\nc\n");
    let data = common::write_file(dir, "extra.txt", "X1\nX2\n");

    common::sedx_isolated(dir)
        .args([
            &format!("2r {}", data.to_str().unwrap()),
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(common::read_file(&input), "a\nb\nX1\nX2\nc\n");
}

#[test]
fn write_file_captures_pattern_space() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = common::write_file(dir, "in.txt", "alpha\nbravo\n");

    common::sedx_isolated(dir)
        .current_dir(dir)
        .args(["/bravo/w captured.txt", input.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(common::read_file(&dir.join("captured.txt")), "bravo\n");
}

#[test]
fn read_line_reads_one_line_per_match() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let data = common::write_file(home.path(), "data.txt", "Z1\nZ2\nZ3\n");

    common::sedx()
        .arg(format!("R {}", data.to_str().unwrap()))
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("a\nZ1\nb\nZ2\nc\nZ3\n");
}

#[test]
fn file_mode_read_line_reads_one_line_per_match() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = common::write_file(dir, "in.txt", "a\nb\nc\n");
    let data = common::write_file(dir, "data.txt", "Z1\nZ2\nZ3\n");

    common::sedx_isolated(dir)
        .args([
            &format!("R {}", data.to_str().unwrap()),
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(common::read_file(&input), "a\nZ1\nb\nZ2\nc\nZ3\n");
}

#[test]
fn write_first_line_captures_first_line_of_pattern_space() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let out = dir.join("first.txt");
    let input = common::write_file(dir, "in.txt", "a\nb\nc\nd\n");

    common::sedx_isolated(dir)
        .args([
            &format!("N; W {}", out.to_str().unwrap()),
            input.to_str().unwrap(),
        ])
        .assert()
        .success();

    // N joins pairs into pattern space; W writes only the first line of each.
    assert_eq!(common::read_file(&out), "a\nc\n");
}

#[test]
fn print_line_number_emits_1_indexed_number() {
    common::sedx()
        .arg("=")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success()
        .stdout("1\na\n2\nb\n3\nc\n");
}

#[test]
fn print_filename_emits_current_input_path() {
    use tempfile::TempDir;
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = common::write_file(dir, "named.txt", "only\n");

    common::sedx_isolated(dir)
        .args(["--dry-run", "F", input.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("named.txt"));
}

#[test]
fn clear_pattern_space_drops_content() {
    // z clears, so the following s/ has nothing to match; line becomes empty.
    common::sedx()
        .arg("z; s/^$/CLEARED/")
        .write_stdin("anything\n")
        .assert()
        .success()
        .stdout("CLEARED\n");
}
