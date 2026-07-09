//! ADR-0014 acceptance: discard + undo round-trips to a byte-identical
//! working tree, the trash entry exists before anything is destroyed,
//! stale hunks are refused with the tree untouched, and the index is
//! never written.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use git2::{Repository, Status};
use margin_core::{render_hunk_patch, render_reversed_hunk_patch};
use margin_vcs::{
    apply_patch_to_worktree, undo_last_discard, write_trash, DiffSource, GitWorktree, StageError,
    UndoError,
};

const BASE: &str = "line one\nline two\nline three\nline four\nline five\nline six\n\
line seven\nline eight\nline nine\nline ten\nline eleven\nline twelve\n\
line thirteen\nline fourteen\nline fifteen\nline sixteen\nline seventeen\n\
line eighteen\nline nineteen\nline twenty\n";

/// Same edit at both ends of the file: far enough apart for two hunks.
const MODIFIED: &str = "line one\nline TWO changed\nline three\nline four\nline five\nline six\n\
line seven\nline eight\nline nine\nline ten\nline eleven\nline twelve\n\
line thirteen\nline fourteen\nline fifteen\nline SIXTEEN changed\nline seventeen\n\
line eighteen\nline nineteen\nline twenty\n";

/// MODIFIED with the first hunk discarded: only the second edit remains.
const FIRST_DISCARDED: &str = "line one\nline two\nline three\nline four\nline five\nline six\n\
line seven\nline eight\nline nine\nline ten\nline eleven\nline twelve\n\
line thirteen\nline fourteen\nline fifteen\nline SIXTEEN changed\nline seventeen\n\
line eighteen\nline nineteen\nline twenty\n";

fn repo_with_two_hunks() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();
    // Worktree applies write through git's checkout filters. Windows
    // runners set core.autocrlf=true globally, which would rewrite LF
    // fixtures as CRLF and break byte-identity — pin it off per-repo.
    config.set_bool("core.autocrlf", false).unwrap();
    fs::write(dir.path().join("notes.txt"), BASE).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("notes.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();
    }
    fs::write(dir.path().join("notes.txt"), MODIFIED).unwrap();
    (dir, repo)
}

fn status_of(repo: &Repository, rel: &str) -> Status {
    repo.statuses(None)
        .unwrap()
        .iter()
        .find(|e| e.path().ok() == Some(rel))
        .map(|e| e.status())
        .unwrap_or(Status::CURRENT)
}

#[test]
fn discard_then_undo_round_trips_byte_identical_and_never_touches_the_index() {
    let (dir, repo) = repo_with_two_hunks();
    let path = dir.path();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = &changeset.files[0];
    assert_eq!(file.hunks.len(), 2, "edits must split into two hunks");

    // ADR-0014 order: the forward copy exists before anything is destroyed.
    let backup = render_hunk_patch(file, &file.hunks[0]).unwrap();
    let trash = write_trash(path, &backup).unwrap();
    assert!(trash.exists(), "trash entry written before the apply");
    assert_eq!(
        fs::read(&trash).unwrap(),
        backup,
        "trash is the exact bytes"
    );

    let reversed = render_reversed_hunk_patch(file, &file.hunks[0]).unwrap();
    apply_patch_to_worktree(path, &reversed).unwrap();
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        FIRST_DISCARDED,
        "only the first hunk was discarded"
    );

    // Discard is worktree-only: the index never saw any of this.
    let st = status_of(&repo, "notes.txt");
    assert!(!st.contains(Status::INDEX_MODIFIED), "index clean: {st:?}");
    assert!(
        st.contains(Status::WT_MODIFIED),
        "second edit remains: {st:?}"
    );

    // Undo restores byte-identical content and consumes the trash entry.
    let restored = undo_last_discard(path).unwrap();
    assert_eq!(restored, trash);
    assert!(!trash.exists(), "consumed on successful undo");
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        MODIFIED,
        "undo round-trips to the byte-identical tree"
    );

    // Nothing left to undo.
    assert!(matches!(undo_last_discard(path), Err(UndoError::Empty)));
}

#[test]
fn stale_discard_is_refused_and_the_tree_untouched() {
    let (dir, _repo) = repo_with_two_hunks();
    let path = dir.path();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = &changeset.files[0];
    let reversed = render_reversed_hunk_patch(file, &file.hunks[0]).unwrap();

    // The world moves after the review loaded — the normal agent case.
    let moved = MODIFIED.replace("line TWO changed", "line TWO changed again");
    fs::write(path.join("notes.txt"), &moved).unwrap();

    let err = apply_patch_to_worktree(path, &reversed).unwrap_err();
    assert!(matches!(err, StageError::Stale(_)), "{err:?}");
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        moved,
        "a refused discard changes nothing"
    );
}

#[test]
fn stale_undo_keeps_the_trash_entry_for_hand_recovery() {
    let (dir, _repo) = repo_with_two_hunks();
    let path = dir.path();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = &changeset.files[0];
    let backup = render_hunk_patch(file, &file.hunks[0]).unwrap();
    let trash = write_trash(path, &backup).unwrap();
    let reversed = render_reversed_hunk_patch(file, &file.hunks[0]).unwrap();
    apply_patch_to_worktree(path, &reversed).unwrap();

    // The discarded region changes again before the undo.
    let moved = FIRST_DISCARDED.replace("line two", "line 2.0");
    fs::write(path.join("notes.txt"), &moved).unwrap();

    let err = undo_last_discard(path).unwrap_err();
    match err {
        UndoError::Stale { path: kept } => {
            assert_eq!(kept, trash);
            assert!(trash.exists(), "stale entries are kept, never dropped");
        }
        other => panic!("expected Stale, got {other:?}"),
    }
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        moved,
        "a refused undo changes nothing"
    );
}

#[test]
fn trash_names_never_overwrite() {
    let (dir, _repo) = repo_with_two_hunks();
    let path = dir.path();
    let a = write_trash(path, b"first").unwrap();
    let b = write_trash(path, b"second").unwrap();
    assert_ne!(a, b, "same-millisecond writes get distinct names");
    assert_eq!(fs::read(&a).unwrap(), b"first");
    assert_eq!(fs::read(&b).unwrap(), b"second");
    // Undo consumes newest-first: b, then a.
    assert!(b > a, "lexical order is trash order");
}

#[test]
fn discarding_an_untracked_file_deletes_it_and_undo_restores_it() {
    let (dir, _repo) = repo_with_two_hunks();
    let path = dir.path();
    fs::write(path.join("scratch.txt"), "temporary notes\n").unwrap();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = changeset
        .files
        .iter()
        .find(|f| f.display_path() == "scratch.txt")
        .expect("untracked files enter the worktree view as additions");

    let backup = render_hunk_patch(file, &file.hunks[0]).unwrap();
    write_trash(path, &backup).unwrap();
    let reversed = render_reversed_hunk_patch(file, &file.hunks[0]).unwrap();
    apply_patch_to_worktree(path, &reversed).unwrap();
    assert!(
        !path.join("scratch.txt").exists(),
        "discarding an addition removes the file"
    );

    undo_last_discard(path).unwrap();
    assert_eq!(
        fs::read_to_string(path.join("scratch.txt")).unwrap(),
        "temporary notes\n",
        "undo recreates the file byte-identical"
    );
}
