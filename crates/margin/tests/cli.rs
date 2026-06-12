//! Binary-level integration tests (ADR-0010 layer 3): exit codes and the
//! pager passthrough guarantee (ADR-0007) — the contracts scripts rely on.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn margin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_margin"))
}

fn run_with_stdin(args: &[&str], stdin_bytes: &[u8]) -> Output {
    let mut child = margin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn margin");
    child
        .stdin
        .take()
        .expect("stdin handle")
        .write_all(stdin_bytes)
        .expect("write stdin");
    child.wait_with_output().expect("wait for margin")
}

/// The contract that makes `core.pager = "margin pager"` safe forever:
/// piped output is byte-identical to the input — including ANSI colors,
/// invalid UTF-8, CRLF, and a missing trailing newline.
#[test]
fn pager_passthrough_is_byte_identical() {
    let hostile: &[u8] =
        b"\x1b[1mdiff --git a/x b/x\x1b[m\n+\xff\xfe bytes\r\n\x1b[32m+green\x1b[m\nno trailing newline";
    let out = run_with_stdin(&["pager"], hostile);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        out.stdout, hostile,
        "passthrough must not alter a single byte"
    );
}

#[test]
fn patch_stdin_passthrough_matches_pager() {
    let patch = b"--- a.txt\n+++ b.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
    let out = run_with_stdin(&["patch", "-"], patch);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, patch.to_vec());
}

#[test]
fn two_file_diff_prints_summary_when_piped() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.txt");
    let new = dir.path().join("new.txt");
    std::fs::write(&old, "one\ntwo\n").unwrap();
    std::fs::write(&new, "one\nTWO\nthree\n").unwrap();

    let out = margin()
        .args(["diff", old.to_str().unwrap(), new.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("+2") && stdout.contains("-1"), "{stdout}");
}

#[test]
fn identical_files_report_no_changes() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    std::fs::write(&a, "same\n").unwrap();
    let out = margin()
        .args(["diff", a.to_str().unwrap(), a.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("no changes"));
}

#[test]
fn outside_a_repo_exits_2_with_a_clear_message() {
    let dir = tempfile::tempdir().unwrap();
    let out = margin().current_dir(dir.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a git repository"));
}

#[test]
fn usage_errors_exit_2() {
    // clap's standard usage-error exit code, promised by ADR-0007.
    let out = margin().args(["frobnicate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));

    let dir = tempfile::tempdir().unwrap();
    let out = margin()
        .current_dir(dir.path())
        .args(["diff", "--staged", "HEAD~1"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn colored_patch_from_file_parses_for_summary() {
    // `margin patch file` to a pipe: parse (ANSI stripped) is exercised via
    // the TTY path normally; when piped it passes through. Verify the file
    // path + passthrough behavior.
    let dir = tempfile::tempdir().unwrap();
    let patch_path = dir.path().join("c.patch");
    let colored = b"\x1b[36m@@ -1,1 +1,1 @@\x1b[m\n-x\n+y\n";
    std::fs::write(&patch_path, colored).unwrap();
    let out = margin()
        .args(["patch", patch_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        out.stdout,
        colored.to_vec(),
        "file patches pass through too"
    );
}

#[test]
fn version_flag_works() {
    let out = margin().arg("--version").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("margin "));
}
