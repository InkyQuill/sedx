mod common;

use common::{read_file, sedx_isolated, write_file};
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
