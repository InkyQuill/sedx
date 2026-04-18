//! Tests for exit codes and error-message surface.

mod common;

use common::{sedx, sedx_isolated, write_file};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn unparseable_expression_exits_nonzero_with_hint() {
    sedx()
        .arg("s/unclosed") // missing trailing delimiter
        .write_stdin("anything\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse expression"))
        .stderr(predicate::str::contains("missing closing delimiter"));
}

#[test]
fn missing_input_file_emits_warning_to_stderr() {
    // Current behavior (codified, not asserted as correct): missing files
    // produce a stderr warning and exit 0 with "No changes would be made."
    // If that policy is tightened to exit non-zero, update this test.
    //
    // The OS error message varies by platform:
    //   Unix:    "No such file or directory"
    //   Windows: "The system cannot find the path specified."
    // We accept either phrasing — the assertion is that the missing-file
    // error is surfaced in stderr, not that any one OS's wording is used.
    let home = TempDir::new().unwrap();
    sedx_isolated(home.path())
        .args(["s/foo/bar/", "/nonexistent/definitely/missing.txt"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Error resolving"))
        .stderr(
            predicate::str::contains("No such file")
                .or(predicate::str::contains("cannot find the path")),
        );
}

#[cfg(unix)]
#[test]
fn unreadable_input_file_surfaces_permission_error() {
    use std::os::unix::fs::PermissionsExt;

    // Running as root bypasses Unix mode bits, which makes this test
    // unreachable. Skip cleanly in that case. `USER`/`LOGNAME` are the
    // portable, std-only signals (no need for an FFI uid call).
    let is_root = std::env::var("USER").as_deref() == Ok("root")
        || std::env::var("LOGNAME").as_deref() == Ok("root");
    if is_root {
        eprintln!("skipping: running as root, permission denied is unreachable");
        return;
    }

    let home = TempDir::new().unwrap();
    let file = write_file(home.path(), "in.txt", "foo\n");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();

    let result = sedx_isolated(home.path())
        .args(["s/foo/bar/", file.to_str().unwrap()])
        .assert();

    // Restore perms so TempDir can clean up, regardless of assertion outcome.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).ok();

    result.stderr(
        predicate::str::contains("Error processing")
            .or(predicate::str::contains("Permission denied"))
            .or(predicate::str::contains("denied")),
    );
}
