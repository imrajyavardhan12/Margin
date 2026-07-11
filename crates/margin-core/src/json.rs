//! The `--json` output schema (issue #22, BLUEPRINT §3.6): how agents and
//! scripts consume Margin without scraping a TTY.
//!
//! These types are a **deliberate public surface**, deliberately decoupled
//! from the internal model: refactoring `model.rs` cannot change the JSON
//! shape without touching this file. The schema is versioned by the
//! top-level `schema` field; within a version, changes are additive only.
//! Documented in `docs/json-output.md`.
//!
//! Encoding decision (bytes-first model → JSON strings): every string is
//! lossy UTF-8, and any value that had unrepresentable bytes carries a
//! `lossy: true` sibling flag. One uniform shape for consumers; the rare
//! non-UTF-8 case is detectable, and exact bytes remain available from
//! the raw diff itself.

use serde::Serialize;

use crate::model::{Changeset, FileDiff, FileStatus, Hunk, Line, LineKind};

/// Current value of the top-level `schema` field.
pub const JSON_SCHEMA_VERSION: u32 = 1;

/// Top-level `--json` document.
#[derive(Debug, Serialize)]
pub struct JsonChangeset {
    pub schema: u32,
    pub files: Vec<JsonFile>,
    /// Total added lines across all files.
    pub additions: usize,
    /// Total deleted lines across all files.
    pub deletions: usize,
}

#[derive(Debug, Serialize)]
pub struct JsonFile {
    /// `added` | `deleted` | `modified` | `renamed` | `copied`
    pub status: &'static str,
    /// Path on the old side; `null` for added files.
    pub old_path: Option<String>,
    /// Path on the new side; `null` for deleted files.
    pub new_path: Option<String>,
    /// True when either path contained non-UTF-8 bytes (replaced with
    /// U+FFFD in the strings above).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub path_lossy: bool,
    pub binary: bool,
    /// Octal file modes as strings (`"100644"`), git-style.
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    /// `similarity index` percentage for renames/copies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<u8>,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<JsonHunk>,
}

#[derive(Debug, Serialize)]
pub struct JsonHunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    /// The `@@ ... @@ heading` section text, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// True when `heading` had non-UTF-8 bytes (replaced with U+FFFD).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub heading_lossy: bool,
    pub lines: Vec<JsonLine>,
}

#[derive(Debug, Serialize)]
pub struct JsonLine {
    /// `context` | `addition` | `deletion`
    pub kind: &'static str,
    /// Line content without its marker or trailing newline.
    pub content: String,
    /// True when `content` had non-UTF-8 bytes (replaced with U+FFFD).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lossy: bool,
    /// True when followed by `\ No newline at end of file`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_newline: bool,
}

/// Build the schema-1 document for a changeset.
pub fn json_changeset(changeset: &Changeset) -> JsonChangeset {
    JsonChangeset {
        schema: JSON_SCHEMA_VERSION,
        files: changeset.files.iter().map(json_file).collect(),
        additions: changeset.additions(),
        deletions: changeset.deletions(),
    }
}

fn json_file(file: &FileDiff) -> JsonFile {
    let (old_path, old_lossy) = optional_string(file.old_path.as_deref());
    let (new_path, new_lossy) = optional_string(file.new_path.as_deref());
    JsonFile {
        status: match file.status {
            FileStatus::Added => "added",
            FileStatus::Deleted => "deleted",
            FileStatus::Modified => "modified",
            FileStatus::Renamed => "renamed",
            FileStatus::Copied => "copied",
        },
        old_path,
        new_path,
        path_lossy: old_lossy || new_lossy,
        binary: file.is_binary,
        old_mode: file.old_mode.map(|m| format!("{m:o}")),
        new_mode: file.new_mode.map(|m| format!("{m:o}")),
        similarity: file.similarity,
        additions: file.additions(),
        deletions: file.deletions(),
        hunks: file.hunks.iter().map(json_hunk).collect(),
    }
}

fn json_hunk(hunk: &Hunk) -> JsonHunk {
    let (heading, heading_lossy) = optional_string(hunk.heading.as_deref());
    JsonHunk {
        old_start: hunk.old_start,
        old_count: hunk.old_count,
        new_start: hunk.new_start,
        new_count: hunk.new_count,
        heading,
        heading_lossy,
        lines: hunk.lines.iter().map(json_line).collect(),
    }
}

fn json_line(line: &Line) -> JsonLine {
    let (content, lossy) = lossy_string(&line.content);
    JsonLine {
        kind: match line.kind {
            LineKind::Context => "context",
            LineKind::Addition => "addition",
            LineKind::Deletion => "deletion",
        },
        content,
        lossy,
        no_newline: line.no_newline,
    }
}

/// Lossy UTF-8 plus the honesty flag.
fn lossy_string(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    }
}

fn optional_string(bytes: Option<&[u8]>) -> (Option<String>, bool) {
    match bytes {
        None => (None, false),
        Some(b) => {
            let (s, lossy) = lossy_string(b);
            (Some(s), lossy)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::parse_unified;

    const MIXED: &str = "\
diff --git a/src/app.rs b/src/app.rs
old mode 100644
new mode 100755
index 1111111..2222222
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,2 +1,2 @@ fn main()
 fn keep() {}
-fn old() {}
+fn new_fn() {}
\\ No newline at end of file
diff --git a/old/name.txt b/new/name.txt
similarity index 90%
rename from old/name.txt
rename to new/name.txt
--- a/old/name.txt
+++ b/new/name.txt
@@ -1,1 +1,1 @@
-a
+b
diff --git a/logo.png b/logo.png
index 3333333..4444444 100644
Binary files a/logo.png and b/logo.png differ
";

    #[test]
    fn schema_covers_renames_binary_and_line_detail() {
        let changeset = parse_unified(MIXED.as_bytes()).changeset;
        let doc = json_changeset(&changeset);
        assert_eq!(doc.schema, 1);
        assert_eq!(doc.files.len(), 3);

        let modified = &doc.files[0];
        assert_eq!(modified.status, "modified");
        assert_eq!(modified.new_path.as_deref(), Some("src/app.rs"));
        assert_eq!(modified.hunks[0].heading.as_deref(), Some("fn main()"));
        let last = modified.hunks[0].lines.last().unwrap();
        assert_eq!((last.kind, last.no_newline), ("addition", true));

        let renamed = &doc.files[1];
        assert_eq!(renamed.status, "renamed");
        assert_eq!(renamed.old_path.as_deref(), Some("old/name.txt"));
        assert_eq!(renamed.new_path.as_deref(), Some("new/name.txt"));
        assert_eq!(renamed.similarity, Some(90));

        let binary = &doc.files[2];
        assert!(binary.binary);
        assert!(binary.hunks.is_empty());
    }

    #[test]
    fn non_utf8_content_is_lossy_flagged_never_dropped() {
        let patch = b"--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n-ok\n+bad \xff\xfe bytes\n";
        let changeset = parse_unified(patch).changeset;
        let doc = json_changeset(&changeset);
        let lines = &doc.files[0].hunks[0].lines;
        assert!(!lines[0].lossy, "clean UTF-8 carries no flag");
        assert!(lines[1].lossy, "replacement characters are flagged");
        assert!(lines[1].content.contains('\u{fffd}'));
        assert!(lines[1].content.contains("bytes"), "the rest survives");
    }

    #[test]
    fn non_utf8_headings_carry_the_flag_too() {
        // Latin-1 source: `@@ ... @@ f\xFCnf()` — the heading is bytes.
        let patch = b"--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@ f\xfcnf()\n-a\n+b\n";
        let changeset = parse_unified(patch).changeset;
        let doc = json_changeset(&changeset);
        let hunk = &doc.files[0].hunks[0];
        assert!(hunk.heading_lossy, "mangled heading must say so");
        assert!(hunk.heading.as_deref().unwrap().contains('\u{fffd}'));

        // And a clean heading carries none.
        let clean = parse_unified(b"--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@ fn ok()\n-a\n+b\n").changeset;
        assert!(!json_changeset(&clean).files[0].hunks[0].heading_lossy);
    }

    #[test]
    fn serializes_to_stable_json_shape() {
        let changeset = parse_unified(MIXED.as_bytes()).changeset;
        let text = serde_json::to_string(&json_changeset(&changeset)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["files"][1]["status"], "renamed");
        assert_eq!(value["files"][2]["binary"], true);
        assert_eq!(value["files"][0]["hunks"][0]["lines"][0]["kind"], "context");
        // Absent-by-default flags stay absent: no noise in the common case.
        assert!(value["files"][0]["hunks"][0]["lines"][0]
            .get("lossy")
            .is_none());
        assert!(value["files"][0].get("path_lossy").is_none());
        // Modes are octal strings, git-style.
        assert_eq!(value["files"][0]["old_mode"], "100644");
        assert_eq!(value["files"][0]["new_mode"], "100755");
    }
}
