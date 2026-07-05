//! ADR-0013 acceptance: stage/unstage round-trips leave `git status`
//! consistent with plain git, and staging never touches the working tree.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use git2::{Repository, Status};
use margin_core::{render_hunk_patch, render_reversed_hunk_patch};
use margin_vcs::{apply_patch_to_index, DiffSource, GitStaged, GitWorktree, StageError};

const BASE: &str = "line one\nline two\nline three\nline four\nline five\nline six\n\
line seven\nline eight\nline nine\nline ten\nline eleven\nline twelve\n\
line thirteen\nline fourteen\nline fifteen\nline sixteen\nline seventeen\n\
line eighteen\nline nineteen\nline twenty\n";

/// Same edit at both ends of the file: far enough apart for two hunks.
const MODIFIED: &str = "line one\nline TWO changed\nline three\nline four\nline five\nline six\n\
line seven\nline eight\nline nine\nline ten\nline eleven\nline twelve\n\
line thirteen\nline fourteen\nline fifteen\nline SIXTEEN changed\nline seventeen\n\
line eighteen\nline nineteen\nline twenty\n";

fn repo_with_two_hunks() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();
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
fn stage_then_unstage_round_trips_and_never_touches_the_worktree() {
    let (dir, repo) = repo_with_two_hunks();
    let path = dir.path();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = &changeset.files[0];
    assert_eq!(file.hunks.len(), 2, "edits must split into two hunks");

    // Stage only the first hunk.
    let patch = render_hunk_patch(file, &file.hunks[0]).unwrap();
    apply_patch_to_index(path, &patch).unwrap();

    // Zero data-loss: the working tree is byte-identical.
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        MODIFIED
    );

    // git status agrees: changes both staged and unstaged.
    let st = status_of(&repo, "notes.txt");
    assert!(st.contains(Status::INDEX_MODIFIED), "staged: {st:?}");
    assert!(st.contains(Status::WT_MODIFIED), "still dirty: {st:?}");

    // The index holds exactly the first hunk's change.
    let staged = GitStaged::new(path).load().unwrap();
    let staged_hunk = &staged.files[0].hunks[0];
    let text = staged_hunk
        .lines
        .iter()
        .map(|l| String::from_utf8_lossy(&l.content).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("line TWO changed"), "{text}");
    assert!(!text.contains("SIXTEEN"), "{text}");

    // Unstage it: reversed patch, index returns to HEAD.
    let reversed = render_reversed_hunk_patch(file, &file.hunks[0]).unwrap();
    apply_patch_to_index(path, &reversed).unwrap();
    let st = status_of(&repo, "notes.txt");
    assert!(!st.contains(Status::INDEX_MODIFIED), "unstaged: {st:?}");
    assert!(st.contains(Status::WT_MODIFIED), "worktree intact: {st:?}");
    assert!(GitStaged::new(path).load().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        MODIFIED
    );
}

#[test]
fn stale_hunk_is_refused_and_the_index_is_untouched() {
    let (dir, _repo) = repo_with_two_hunks();
    let path = dir.path();

    let changeset = GitWorktree::new(path).load().unwrap();
    let file = &changeset.files[0];
    let patch = render_hunk_patch(file, &file.hunks[0]).unwrap();

    apply_patch_to_index(path, &patch).unwrap();
    let before = GitStaged::new(path).load().unwrap();

    // The same hunk no longer applies (it is already in the index):
    // the reviewed diff is stale relative to the index.
    let err = apply_patch_to_index(path, &patch).unwrap_err();
    assert!(matches!(err, StageError::Stale(_)), "{err:?}");

    // And the failed attempt changed nothing.
    assert_eq!(GitStaged::new(path).load().unwrap(), before);
}
