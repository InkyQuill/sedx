#![allow(dead_code)] // Each integration-test binary uses a subset of these helpers.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// Returns a `sedx` invocation with ANSI color disabled so stdout/stderr
/// assertions can compare plain text.
pub fn sedx() -> Command {
    let mut cmd = Command::cargo_bin("sedx").expect("sedx binary built by cargo");
    cmd.env("NO_COLOR", "1");
    cmd
}

/// Returns a `sedx` invocation pre-wired to resolve `~` (and therefore
/// `~/.sedx/backups/`) to the given directory, on every platform. Use this
/// for any test that exercises file-mode editing, backups, `history`,
/// or `rollback` so real `~/.sedx/` is never touched.
pub fn sedx_isolated(home_dir: &Path) -> Command {
    let mut cmd = sedx();
    // dirs::home_dir() reads $HOME on Unix and %USERPROFILE% on Windows.
    // Setting both is portable and idempotent.
    cmd.env("HOME", home_dir);
    cmd.env("USERPROFILE", home_dir);
    cmd
}

/// Writes `content` to `dir/name` and returns the full path.
pub fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write file");
    path
}

/// Reads a file to a String.
pub fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read file")
}
