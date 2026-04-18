//! Tests that the streaming processor produces byte-exact output on both
//! small files (forced via --streaming) and large files (#[ignore]'d).

mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[test]
fn forced_streaming_small_file_matches_in_memory() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input: String = (0..1000)
        .map(|i| format!("line {} foo\n", i))
        .collect();

    let in_mem = write_file(dir, "mem.txt", &input);
    sedx_isolated(dir)
        .args(["--no-streaming", "s/foo/bar/", in_mem.to_str().unwrap()])
        .assert()
        .success();

    let streamed = write_file(dir, "stream.txt", &input);
    sedx_isolated(dir)
        .args(["--streaming", "s/foo/bar/", streamed.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(&in_mem), read_file(&streamed));
}

#[cfg(unix)]
#[test]
fn streaming_via_symlink_writes_to_target() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "foo\nbar\nbaz\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(dir)
        .args(["--streaming", "s/bar/BAR/", link.to_str().unwrap()])
        .assert()
        .success();

    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(read_file(&target), "foo\nBAR\nbaz\n");
}

/// Opt-in via `cargo test -- --ignored`. Generates a ~100 MB input file and
/// verifies that every line was transformed correctly. Takes ~5 s; the
/// #[ignore] keeps routine CI fast.
#[test]
#[ignore]
fn streaming_100mb_file_correctness() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let path = dir.join("big.txt");

    // 1 million lines × ~100 bytes each ≈ 100 MB.
    let mut contents = String::with_capacity(100 * 1024 * 1024);
    for i in 0..1_000_000 {
        contents.push_str(&format!(
            "line {:07} foo {:0>50}\n",
            i, "x"
        ));
    }
    std::fs::write(&path, &contents).unwrap();

    sedx_isolated(dir)
        .args(["s/foo/bar/", path.to_str().unwrap()])
        .assert()
        .success();

    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result.matches(" foo ").count(), 0);
    assert_eq!(result.matches(" bar ").count(), 1_000_000);
}
