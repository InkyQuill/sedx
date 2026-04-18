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

/// Returns a `sedx` invocation pre-wired to resolve sedx's state dir
/// (normally `~/.sedx/`) to the given directory, on every platform. Use
/// this for any test that exercises file-mode editing, backups, `history`,
/// or `rollback` so real `~/.sedx/` is never touched.
///
/// Uses `SEDX_HOME` rather than `HOME`/`USERPROFILE` because on Windows
/// `dirs::home_dir()` reads the shell API (`SHGetKnownFolderPath`) and
/// ignores env vars. sedx itself checks `SEDX_HOME` before falling back
/// to `dirs::home_dir()` — see `backup_manager::sedx_home`.
pub fn sedx_isolated(home_dir: &Path) -> Command {
    let mut cmd = sedx();
    cmd.env("SEDX_HOME", home_dir);
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
