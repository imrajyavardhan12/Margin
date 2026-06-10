//! Integration tests for the git2-backed sources (ADR-0010 layer 3):
//! build real repositories in temp dirs and assert the produced changesets.

// Test helpers may panic freely; the no-panic policy (ADR-0009) applies to
// library code, not to test scaffolding.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use git2::{Repository, Signature};
use margin_core::FileStatus;
use margin_vcs::{DiffSource, GitRevRange, GitShow, GitStaged, GitWorktree, SourceError};
use std::fs;
use std::path::Path;

struct TestRepo {
    dir: tempfile::TempDir,
    repo: Repository,
}

impl TestRepo {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let repo = Repository::init(dir.path()).expect("git init");
        let mut config = repo.config().expect("repo config");
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        Self { dir, repo }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, content: impl AsRef<[u8]>) {
        let path = self.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn stage(&self, rel: &str) {
        let mut index = self.repo.index().unwrap();
        index.add_path(Path::new(rel)).unwrap();
        index.write().unwrap();
    }

    fn unstage_remove(&self, rel: &str) {
        let mut index = self.repo.index().unwrap();
        index.remove_path(Path::new(rel)).unwrap();
        index.write().unwrap();
        fs::remove_file(self.path().join(rel)).unwrap();
    }

    fn commit(&self, msg: &str) -> String {
        let mut index = self.repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
            .to_string()
    }
}

#[test]
fn worktree_shows_modified_and_untracked_with_content() {
    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    t.commit("initial");

    t.write("a.txt", "one\ntwo\n"); // modified, unstaged
    t.write("brand-new.txt", "fresh\n"); // untracked

    let cs = GitWorktree::new(t.path()).load().unwrap();
    assert_eq!(cs.files.len(), 2, "modified + untracked");

    let modified = cs
        .files
        .iter()
        .find(|f| f.display_path() == "a.txt")
        .expect("a.txt present");
    assert_eq!(modified.status, FileStatus::Modified);
    assert_eq!((modified.additions(), modified.deletions()), (1, 0));

    let untracked = cs
        .files
        .iter()
        .find(|f| f.display_path() == "brand-new.txt")
        .expect("untracked file present");
    assert_eq!(untracked.status, FileStatus::Added);
    assert_eq!(untracked.old_path, None);
    let line = &untracked.hunks[0].lines[0];
    assert_eq!(
        line.content,
        b"fresh".to_vec(),
        "untracked content rendered"
    );
}

#[test]
fn worktree_can_exclude_untracked() {
    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    t.commit("initial");
    t.write("loose.txt", "x\n");

    let mut source = GitWorktree::new(t.path());
    source.include_untracked = false;
    let cs = source.load().unwrap();
    assert!(cs.is_empty(), "untracked excluded on request");
}

#[test]
fn staged_reflects_index_only() {
    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    t.commit("initial");

    t.write("a.txt", "one\nstaged-line\n");
    t.stage("a.txt");
    t.write("a.txt", "one\nstaged-line\nunstaged-line\n");

    let staged = GitStaged::new(t.path()).load().unwrap();
    assert_eq!(staged.files.len(), 1);
    assert_eq!(staged.additions(), 1, "only the staged addition");

    let worktree = GitWorktree::new(t.path()).load().unwrap();
    assert_eq!(worktree.additions(), 2, "worktree vs HEAD sees both");
}

#[test]
fn show_diffs_commit_against_parent_and_handles_root() {
    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    let root = t.commit("initial");

    t.write("a.txt", "two\n");
    t.stage("a.txt");
    t.commit("change");

    let head = GitShow::new(t.path(), "HEAD").load().unwrap();
    assert_eq!(head.files.len(), 1);
    assert_eq!((head.additions(), head.deletions()), (1, 1));

    let first = GitShow::new(t.path(), root.as_str()).load().unwrap();
    assert_eq!(first.files.len(), 1);
    assert_eq!(
        first.files[0].status,
        FileStatus::Added,
        "root commit = all added"
    );
}

#[test]
fn rev_range_diffs_two_commits() {
    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    let c1 = t.commit("first");
    t.write("b.txt", "bee\n");
    t.stage("b.txt");
    let c2 = t.commit("second");

    let cs = GitRevRange::new(t.path(), c1.as_str(), c2.as_str())
        .load()
        .unwrap();
    assert_eq!(cs.files.len(), 1);
    assert_eq!(cs.files[0].status, FileStatus::Added);
    assert_eq!(cs.files[0].display_path(), "b.txt");
}

#[test]
fn renames_are_detected() {
    let t = TestRepo::new();
    let body = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\n";
    t.write("old-name.txt", body);
    t.stage("old-name.txt");
    let c1 = t.commit("add file");

    t.unstage_remove("old-name.txt");
    t.write("new-name.txt", body);
    t.stage("new-name.txt");
    let c2 = t.commit("rename file");

    let cs = GitRevRange::new(t.path(), c1.as_str(), c2.as_str())
        .load()
        .unwrap();
    assert_eq!(cs.files.len(), 1, "rename collapses delete+add");
    let file = &cs.files[0];
    assert_eq!(file.status, FileStatus::Renamed);
    assert_eq!(file.old_path.as_deref(), Some(&b"old-name.txt"[..]));
    assert_eq!(file.new_path.as_deref(), Some(&b"new-name.txt"[..]));
}

#[test]
fn binary_files_are_flagged_not_hunked() {
    let t = TestRepo::new();
    t.write("blob.bin", [0u8, 159, 146, 150, 0, 255, 1, 2].as_slice());
    t.stage("blob.bin");
    t.commit("binary");

    let cs = GitShow::new(t.path(), "HEAD").load().unwrap();
    assert_eq!(cs.files.len(), 1);
    assert!(cs.files[0].is_binary);
    assert!(cs.files[0].hunks.is_empty());
}

#[test]
fn no_newline_at_eof_is_marked() {
    let t = TestRepo::new();
    t.write("end.txt", "first\nsecond");
    t.stage("end.txt");
    t.commit("no trailing newline");
    t.write("end.txt", "first\nsecond!");

    let cs = GitWorktree::new(t.path()).load().unwrap();
    let lines = &cs.files[0].hunks[0].lines;
    let deletion = lines
        .iter()
        .find(|l| l.content == b"second".to_vec())
        .unwrap();
    let addition = lines
        .iter()
        .find(|l| l.content == b"second!".to_vec())
        .unwrap();
    assert!(deletion.no_newline);
    assert!(addition.no_newline);
}

#[test]
fn unborn_head_treats_everything_as_added() {
    let t = TestRepo::new(); // no commits at all
    t.write("first.txt", "hello\n");

    let cs = GitWorktree::new(t.path()).load().unwrap();
    assert_eq!(cs.files.len(), 1);
    assert_eq!(cs.files[0].status, FileStatus::Added);
}

#[test]
fn errors_are_typed() {
    let dir = tempfile::tempdir().unwrap();
    let err = GitWorktree::new(dir.path()).load().unwrap_err();
    assert!(matches!(err, SourceError::NotARepository { .. }), "{err}");

    let t = TestRepo::new();
    t.write("a.txt", "one\n");
    t.stage("a.txt");
    t.commit("initial");
    let err = GitShow::new(t.path(), "no-such-rev").load().unwrap_err();
    assert!(matches!(err, SourceError::BadRevspec { .. }), "{err}");
}

#[test]
fn diff_ids_are_stable_and_distinct() {
    let t = TestRepo::new();
    let worktree = GitWorktree::new(t.path());
    assert_eq!(worktree.id(), GitWorktree::new(t.path()).id());
    assert_ne!(worktree.id(), GitStaged::new(t.path()).id());
}
