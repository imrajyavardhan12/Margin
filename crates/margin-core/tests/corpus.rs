//! Corpus regression tests (ADR-0010).
//!
//! Every fixture in `tests/corpus/` is a real-world-shaped patch. The rule:
//! any patch that ever breaks Margin gets a fixture here *before* the fix
//! lands, so it can never break again. All fixtures must parse with zero
//! warnings unless the expectation says otherwise.

use margin_core::{parse_unified, Changeset, FileStatus, ParseOutcome};

fn parse_fixture(name: &str) -> ParseOutcome {
    let path = format!("{}/tests/corpus/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"));
    parse_unified(&bytes)
}

fn parse_clean(name: &str) -> Changeset {
    let outcome = parse_fixture(name);
    assert_eq!(outcome.warnings, vec![], "{name}: unexpected warnings");
    outcome.changeset
}

#[test]
fn simple_modify() {
    let cs = parse_clean("simple-modify.patch");
    assert_eq!(cs.files.len(), 1);
    let file = &cs.files[0];
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!(file.display_path(), "src/main.rs");
    assert_eq!(file.hunks.len(), 2);
    assert_eq!(file.hunks[0].heading.as_deref(), Some(&b"fn main()"[..]));
    assert_eq!(file.hunks[1].heading, None);
    assert_eq!((file.additions(), file.deletions()), (4, 2));
}

#[test]
fn add_and_delete() {
    let cs = parse_clean("add-delete.patch");
    assert_eq!(cs.files.len(), 2);
    let added = &cs.files[0];
    assert_eq!(added.status, FileStatus::Added);
    assert_eq!(added.old_path, None);
    assert_eq!(added.display_path(), "NEW.md");
    assert_eq!(added.new_mode, Some(0o100644));
    assert_eq!((added.additions(), added.deletions()), (2, 0));

    let deleted = &cs.files[1];
    assert_eq!(deleted.status, FileStatus::Deleted);
    assert_eq!(deleted.new_path, None);
    assert_eq!(deleted.display_path(), "OLD.md");
    assert_eq!((deleted.additions(), deleted.deletions()), (0, 1));
}

#[test]
fn renames_with_and_without_edits() {
    let cs = parse_clean("rename.patch");
    assert_eq!(cs.files.len(), 2);
    let pure = &cs.files[0];
    assert_eq!(pure.status, FileStatus::Renamed);
    assert_eq!(pure.old_path.as_deref(), Some(&b"docs/old-name.md"[..]));
    assert_eq!(pure.new_path.as_deref(), Some(&b"docs/new-name.md"[..]));
    assert_eq!(pure.similarity, Some(100));
    assert!(pure.hunks.is_empty());

    let edited = &cs.files[1];
    assert_eq!(edited.status, FileStatus::Renamed);
    assert_eq!(edited.similarity, Some(90));
    assert_eq!(edited.hunks.len(), 1);
    assert_eq!((edited.additions(), edited.deletions()), (1, 1));
}

#[test]
fn mode_change_without_content() {
    let cs = parse_clean("mode-change.patch");
    assert_eq!(cs.files.len(), 1);
    let file = &cs.files[0];
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!(file.old_mode, Some(0o100644));
    assert_eq!(file.new_mode, Some(0o100755));
    assert!(file.hunks.is_empty());
}

#[test]
fn binary_files_both_notations() {
    let cs = parse_clean("binary.patch");
    assert_eq!(cs.files.len(), 2);
    assert!(cs.files.iter().all(|f| f.is_binary && f.hunks.is_empty()));
    assert_eq!(cs.files[0].status, FileStatus::Modified);
    assert_eq!(cs.files[1].status, FileStatus::Added);
}

#[test]
fn quoted_unicode_path() {
    let cs = parse_clean("quoted-path.patch");
    assert_eq!(cs.files.len(), 1);
    let file = &cs.files[0];
    assert_eq!(file.display_path(), "notes/café plan.md");
    assert_eq!((file.additions(), file.deletions()), (1, 1));
}

#[test]
fn no_newline_markers() {
    let cs = parse_clean("no-newline.patch");
    let lines = &cs.files[0].hunks[0].lines;
    assert_eq!(lines.len(), 3);
    assert!(!lines[0].no_newline, "context line has a newline");
    assert!(lines[1].no_newline, "deletion lacks trailing newline");
    assert!(lines[2].no_newline, "addition lacks trailing newline");
}

#[test]
fn plain_unified_with_timestamps() {
    let cs = parse_clean("plain-unified.patch");
    assert_eq!(cs.files.len(), 1);
    let file = &cs.files[0];
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!(file.old_path.as_deref(), Some(&b"before.txt"[..]));
    assert_eq!(file.new_path.as_deref(), Some(&b"after.txt"[..]));
    assert_eq!((file.additions(), file.deletions()), (1, 1));
}

#[test]
fn git_log_output_with_commit_headers() {
    let cs = parse_clean("log-with-junk.patch");
    assert_eq!(cs.files.len(), 1, "commit headers must not become files");
    let file = &cs.files[0];
    assert_eq!(file.display_path(), "hello.txt");
    // `@@ -1 +1 @@` form: counts default to 1.
    assert_eq!(file.hunks[0].old_count, 1);
    assert_eq!(file.hunks[0].new_count, 1);
}

#[test]
fn every_fixture_parses_without_panicking() {
    let dir = format!("{}/tests/corpus", env!("CARGO_MANIFEST_DIR"));
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read corpus dir: {e}"));
    let mut count = 0;
    for entry in entries.flatten() {
        let bytes = std::fs::read(entry.path())
            .unwrap_or_else(|e| panic!("cannot read {:?}: {e}", entry.path()));
        let _ = parse_unified(&bytes);
        // Tolerance smoke: parsing any fixture with each byte-prefix removed
        // must not panic either.
        let _ = parse_unified(bytes.get(1..).unwrap_or(b""));
        let _ = parse_unified(bytes.get(..bytes.len() / 2).unwrap_or(b""));
        count += 1;
    }
    assert!(count >= 9, "expected the full corpus, found {count} files");
}
