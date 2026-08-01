//! Markdown export for review notes (issue #23).
//!
//! Pure formatting over the model, like [`crate::json`]: the binary loads
//! the persisted notes and the changeset, this turns them into a document
//! you can paste into a pull request or hand to an agent. Every note
//! carries a `path:line` anchor and its hunk header, because a remark
//! without its location is useless to whoever reads it next.
//!
//! Notes are matched to hunks the same way the TUI matches them — by
//! `(path, hunk digest)` — so a note whose hunk has changed since it was
//! written is silently dropped rather than printed against new code.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::digest::hunk_digest;
use crate::model::Changeset;

/// Render persisted notes as Markdown. `notes` are
/// `(path bytes, hunk digest, text)` straight from the store.
pub fn notes_markdown(changeset: &Changeset, notes: &[(Vec<u8>, u64, String)]) -> String {
    let stored: HashMap<(&[u8], u64), &str> = notes
        .iter()
        .map(|(path, digest, text)| ((path.as_slice(), *digest), text.as_str()))
        .collect();

    let mut out = String::from("# Review notes\n");
    let mut total = 0usize;
    let mut files = 0usize;
    let mut body = String::new();

    for file in &changeset.files {
        let Some(key) = file.new_path.as_deref().or(file.old_path.as_deref()) else {
            continue;
        };
        let path = file.display_path();
        let mut section = String::new();
        for hunk in &file.hunks {
            let Some(text) = stored.get(&(key, hunk_digest(hunk))) else {
                continue;
            };
            total += 1;
            // The anchor is the hunk's first line in the new file — what
            // a reader (or an agent) needs to jump there.
            let _ = write!(section, "\n**{path}:{}**", hunk.new_start);
            let _ = write!(
                section,
                " \u{2014} `@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            );
            if let Some(heading) = &hunk.heading {
                let _ = write!(section, " {}", crate::printable(heading));
            }
            section.push_str("`\n\n");
            // The note text is user input rendered into a document: strip
            // control characters exactly as the TUI does (SECURITY.md).
            section.push_str(&crate::printable(text.as_bytes()));
            section.push('\n');
        }
        if !section.is_empty() {
            files += 1;
            let _ = write!(body, "\n## {path}\n{section}");
        }
    }

    if total == 0 {
        out.push_str("\n_No notes recorded for this review._\n");
        return out;
    }
    let _ = writeln!(
        out,
        "\n_{total} note{} across {files} file{}._",
        if total == 1 { "" } else { "s" },
        if files == 1 { "" } else { "s" }
    );
    out.push_str(&body);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::patch::parse_unified;

    const SAMPLE: &[u8] = b"diff --git a/src/app.rs b/src/app.rs\n\
--- a/src/app.rs\n+++ b/src/app.rs\n\
@@ -1,2 +1,2 @@ fn setup()\n one\n-two\n+TWO\n\
@@ -20,1 +21,1 @@\n-old\n+new\n";

    fn notes_for(changeset: &Changeset, picks: &[(usize, &str)]) -> Vec<(Vec<u8>, u64, String)> {
        picks
            .iter()
            .map(|(hunk, text)| {
                (
                    b"src/app.rs".to_vec(),
                    hunk_digest(&changeset.files[0].hunks[*hunk]),
                    (*text).to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn export_carries_path_line_and_hunk_header() {
        let changeset = parse_unified(SAMPLE).changeset;
        let md = notes_markdown(&changeset, &notes_for(&changeset, &[(0, "why TWO?")]));
        assert!(md.contains("# Review notes"), "{md}");
        assert!(md.contains("_1 note across 1 file._"), "{md}");
        assert!(md.contains("## src/app.rs"), "{md}");
        assert!(md.contains("**src/app.rs:1**"), "anchor line: {md}");
        assert!(
            md.contains("`@@ -1,2 +1,2 @@ fn setup()`"),
            "hunk context: {md}"
        );
        assert!(md.contains("why TWO?"), "{md}");
    }

    #[test]
    fn notes_appear_in_file_and_hunk_order() {
        let changeset = parse_unified(SAMPLE).changeset;
        let md = notes_markdown(
            &changeset,
            &notes_for(&changeset, &[(1, "second hunk"), (0, "first hunk")]),
        );
        let first = md.find("first hunk").expect("first note present");
        let second = md.find("second hunk").expect("second note present");
        assert!(first < second, "document order follows the diff: {md}");
        assert!(md.contains("_2 notes across 1 file._"), "{md}");
    }

    #[test]
    fn stale_notes_are_dropped_not_misattributed() {
        let changeset = parse_unified(SAMPLE).changeset;
        let stale = vec![(
            b"src/app.rs".to_vec(),
            0xdead_beef,
            "written against older code".to_string(),
        )];
        let md = notes_markdown(&changeset, &stale);
        assert!(!md.contains("older code"), "{md}");
        assert!(md.contains("_No notes recorded"), "{md}");
    }

    #[test]
    fn control_characters_in_a_note_cannot_reach_the_document() {
        let changeset = parse_unified(SAMPLE).changeset;
        let md = notes_markdown(
            &changeset,
            &notes_for(&changeset, &[(0, "clear\u{1b}[31m the screen")]),
        );
        assert!(!md.contains('\u{1b}'), "escape byte survived: {md:?}");
        assert!(md.contains("clear"), "{md}");
    }
}
