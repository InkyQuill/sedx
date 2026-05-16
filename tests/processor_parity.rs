mod common;

use common::{read_file, sedx, sedx_isolated, write_file};
use tempfile::TempDir;

#[test]
fn no_streaming_pattern_range_delete_removes_middle_lines() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "range.txt", "AAA\nSTART\nBBB\nEND\nCCC\n");

    sedx_isolated(dir)
        .args([
            "--no-backup",
            "--force",
            "--no-streaming",
            "/START/,/END/d",
            file.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(&file), "AAA\nCCC\n");
}

#[test]
fn duplicate_pattern_ranges_do_not_share_in_memory_state() {
    sedx()
        .args(["-n", "--no-streaming", "/START/,/END/p; /START/,/END/p"])
        .write_stdin("AAA\nSTART\nBBB\nEND\nCCC\n")
        .assert()
        .success()
        .stdout("START\nSTART\nBBB\nBBB\nEND\nEND\n");
}

#[test]
fn duplicate_line_ranges_do_not_share_in_memory_state() {
    sedx()
        .args(["-n", "--no-streaming", "2,4p; 2,4p"])
        .write_stdin("a\nb\nc\nd\ne\n")
        .assert()
        .success()
        .stdout("b\nb\nc\nc\nd\nd\n");
}

#[test]
fn same_pattern_range_uses_normal_in_memory_range_state() {
    sedx()
        .args(["-n", "--no-streaming", "/MARK/,/MARK/p"])
        .write_stdin("before\nMARK\nmiddle\nMARK\nafter\n")
        .assert()
        .success()
        .stdout("MARK\nmiddle\nMARK\n");
}

#[test]
fn reversed_line_range_matches_only_start_line_in_memory() {
    sedx()
        .args(["-n", "--no-streaming", "4,2p"])
        .write_stdin("one\ntwo\nthree\nfour\nfive\n")
        .assert()
        .success()
        .stdout("four\n");
}
