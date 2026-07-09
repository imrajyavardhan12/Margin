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
fn dump_config_reflects_files_and_flags() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("config.toml");
    std::fs::write(&user, "theme = \"carbon\"\nlayout = \"split\"\n").unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(repo.join(".margin.toml"), "theme = \"blueprint\"\n").unwrap();

    // Repo config overrides the user file for display options.
    let out = margin()
        .current_dir(&repo)
        .env("MARGIN_CONFIG", &user)
        .arg("--dump-config")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let dump = String::from_utf8_lossy(&out.stdout);
    assert!(dump.contains("theme = \"blueprint\""), "{dump}");
    assert!(dump.contains("layout = \"split\""), "{dump}");

    // Flags beat both files.
    let out = margin()
        .current_dir(&repo)
        .env("MARGIN_CONFIG", &user)
        .args([
            "--theme",
            "foolscap",
            "--layout",
            "unified",
            "--dump-config",
        ])
        .output()
        .unwrap();
    let dump = String::from_utf8_lossy(&out.stdout);
    assert!(dump.contains("theme = \"foolscap\""), "{dump}");
    assert!(dump.contains("layout = \"unified\""), "{dump}");
}

#[test]
fn config_typos_error_with_suggestions() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("config.toml");
    std::fs::write(&user, "them = \"carbon\"\n").unwrap();
    let out = margin()
        .current_dir(dir.path())
        .env("MARGIN_CONFIG", &user)
        .arg("--dump-config")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("them") && stderr.contains("theme"),
        "{stderr}"
    );
}

#[test]
fn repo_config_cannot_set_behavior_keys() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    std::fs::write(
        dir.path().join(".margin.toml"),
        "include_untracked = false\n",
    )
    .unwrap();
    let out = margin()
        .current_dir(dir.path())
        .env("MARGIN_CONFIG", dir.path().join("none.toml"))
        .arg("--dump-config")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "ADR-0008 trust rule");
}

#[test]
fn unknown_theme_exits_2_listing_builtins() {
    let patch = b"--- a\n+++ b\n@@ -1,1 +1,1 @@\n-x\n+y\n";
    let dir = tempfile::tempdir().unwrap();
    let out = margin()
        .current_dir(dir.path())
        .env("MARGIN_CONFIG", dir.path().join("none.toml"))
        .args(["--theme", "nope", "patch", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(|mut child| {
            child.stdin.take().unwrap().write_all(patch).unwrap();
            child.wait_with_output().unwrap()
        })
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown theme") && stderr.contains("ledger"),
        "{stderr}"
    );
}

#[test]
fn version_flag_works() {
    let out = margin().arg("--version").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("margin "));
}

/// Build a repo with one committed file so `margin undo` has an index
/// and worktree to operate on.
fn undo_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();
    config.set_bool("core.autocrlf", false).unwrap();
    std::fs::write(dir.path().join("f.txt"), "one\ntwo\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("f.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
        .unwrap();
    dir
}

/// `margin undo` restores the newest trash entry and consumes it;
/// empty trash and non-repos exit 2 with the reason (ADR-0007/0014).
#[test]
fn undo_restores_the_seeded_trash_entry() {
    let dir = undo_repo();

    // Empty trash: exit 2, honest message.
    let out = margin()
        .current_dir(dir.path())
        .arg("undo")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nothing to undo"));

    // Seed a trash entry as a discard would have written it: the forward
    // patch of an edit whose reverse has already been applied (i.e. the
    // worktree is at HEAD and the patch re-applies the edit).
    let trash = dir.path().join(".git/margin/trash");
    std::fs::create_dir_all(&trash).unwrap();
    let patch = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n\
                 @@ -1,2 +1,2 @@\n one\n-two\n+TWO\n";
    std::fs::write(trash.join("0000000000001.patch"), patch).unwrap();

    let out = margin()
        .current_dir(dir.path())
        .arg("undo")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("restored"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "one\nTWO\n",
        "the discarded edit is back"
    );
    assert!(
        std::fs::read_dir(&trash).unwrap().next().is_none(),
        "the entry is consumed"
    );
}

#[test]
fn undo_outside_a_repo_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let out = margin()
        .current_dir(dir.path())
        .arg("undo")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
