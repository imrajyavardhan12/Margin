//! Render a single reviewed hunk back into patch bytes (ADR-0013).
//!
//! The inverse of `patch.rs` for exactly one hunk: staging must apply the
//! bytes the reviewer saw, so the patch is reconstructed from the parsed
//! model, never recomputed from the file system. `reversed_*` produce the
//! unstage patch textually — swapping each line's role preserves both
//! side-projections of the hunk, which is all `git apply` consumes.

use crate::model::{FileDiff, FileStatus, Hunk, LineKind};

/// Why a hunk can't be rendered for staging. These are refusals, not
/// failures: the caller disables the action and says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderRefusal {
    /// Binary files have no hunks to stage.
    Binary,
    /// Renames/copies need header semantics v1 does not guess at.
    Rename,
    /// A path contains bytes that would need git's quoting rules
    /// (control chars, quotes, backslashes) — refuse rather than
    /// risk a malformed patch.
    UnsafePath,
}

/// Render `hunk` of `file` as a minimal single-hunk git patch that
/// `git apply` (or libgit2 `Diff::from_buffer`) accepts.
pub fn render_hunk_patch(file: &FileDiff, hunk: &Hunk) -> Result<Vec<u8>, RenderRefusal> {
    if file.is_binary {
        return Err(RenderRefusal::Binary);
    }
    if matches!(file.status, FileStatus::Renamed | FileStatus::Copied) {
        return Err(RenderRefusal::Rename);
    }
    let old = file.old_path.as_deref();
    let new = file.new_path.as_deref();
    for path in [old, new].into_iter().flatten() {
        if path
            .iter()
            .any(|&b| b < 0x20 || b == b'"' || b == b'\\' || b == 0x7f)
        {
            return Err(RenderRefusal::UnsafePath);
        }
    }
    // `diff --git` wants a name on both sides even for add/delete.
    let label = old.or(new).unwrap_or(b"<unknown>");

    let mut out = Vec::with_capacity(256 + hunk.lines.len() * 48);
    out.extend_from_slice(b"diff --git a/");
    out.extend_from_slice(label);
    out.extend_from_slice(b" b/");
    out.extend_from_slice(label);
    out.push(b'\n');
    match file.status {
        FileStatus::Added => {
            let mode = file.new_mode.unwrap_or(0o100644);
            out.extend_from_slice(format!("new file mode {mode:o}\n").as_bytes());
        }
        FileStatus::Deleted => {
            let mode = file.old_mode.unwrap_or(0o100644);
            out.extend_from_slice(format!("deleted file mode {mode:o}\n").as_bytes());
        }
        _ => {}
    }
    match old {
        Some(path) => {
            out.extend_from_slice(b"--- a/");
            out.extend_from_slice(path);
        }
        None => out.extend_from_slice(b"--- /dev/null"),
    }
    out.push(b'\n');
    match new {
        Some(path) => {
            out.extend_from_slice(b"+++ b/");
            out.extend_from_slice(path);
        }
        None => out.extend_from_slice(b"+++ /dev/null"),
    }
    out.push(b'\n');

    out.extend_from_slice(
        format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        )
        .as_bytes(),
    );
    for line in &hunk.lines {
        out.push(match line.kind {
            LineKind::Context => b' ',
            LineKind::Addition => b'+',
            LineKind::Deletion => b'-',
        });
        out.extend_from_slice(&line.content);
        out.push(b'\n');
        if line.no_newline {
            out.extend_from_slice(b"\\ No newline at end of file\n");
        }
    }
    Ok(out)
}

/// The unstage patch: the same hunk with every role swapped, so applying
/// it to the index undoes a prior stage. Involutive: reversing twice
/// yields the original.
pub fn render_reversed_hunk_patch(file: &FileDiff, hunk: &Hunk) -> Result<Vec<u8>, RenderRefusal> {
    render_hunk_patch(&reversed_file(file), &reversed_hunk(hunk))
}

fn reversed_hunk(hunk: &Hunk) -> Hunk {
    let mut rev = hunk.clone();
    rev.old_start = hunk.new_start;
    rev.old_count = hunk.new_count;
    rev.new_start = hunk.old_start;
    rev.new_count = hunk.old_count;
    for line in &mut rev.lines {
        line.kind = match line.kind {
            LineKind::Addition => LineKind::Deletion,
            LineKind::Deletion => LineKind::Addition,
            LineKind::Context => LineKind::Context,
        };
    }
    rev
}

fn reversed_file(file: &FileDiff) -> FileDiff {
    let mut rev = file.clone();
    rev.old_path = file.new_path.clone();
    rev.new_path = file.old_path.clone();
    rev.old_mode = file.new_mode;
    rev.new_mode = file.old_mode;
    rev.status = match file.status {
        FileStatus::Added => FileStatus::Deleted,
        FileStatus::Deleted => FileStatus::Added,
        other => other,
    };
    rev
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::parse_unified;

    const PATCH: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@
 fn keep() {}
-fn old() {}
+fn new_fn() {}
";

    fn one_file() -> FileDiff {
        parse_unified(PATCH.as_bytes()).changeset.files.remove(0)
    }

    #[test]
    fn rendered_hunk_reparses_to_the_same_content() {
        let file = one_file();
        let bytes = render_hunk_patch(&file, &file.hunks[0]).unwrap();
        let reparsed = parse_unified(&bytes);
        assert!(reparsed.warnings.is_empty(), "{:?}", reparsed.warnings);
        let round = &reparsed.changeset.files[0];
        assert_eq!(round.hunks[0].lines, file.hunks[0].lines);
        assert_eq!(round.old_path, file.old_path);
        assert_eq!(round.new_path, file.new_path);
    }

    #[test]
    fn reverse_is_involutive_and_swaps_roles() {
        let file = one_file();
        let rev = reversed_hunk(&file.hunks[0]);
        assert_eq!(rev.old_start, file.hunks[0].new_start);
        assert!(rev
            .lines
            .iter()
            .zip(&file.hunks[0].lines)
            .all(|(r, o)| match o.kind {
                LineKind::Addition => r.kind == LineKind::Deletion,
                LineKind::Deletion => r.kind == LineKind::Addition,
                LineKind::Context => r.kind == LineKind::Context,
            }));
        assert_eq!(reversed_hunk(&rev), file.hunks[0]);
        assert_eq!(reversed_file(&reversed_file(&file)), file);
    }

    #[test]
    fn refusals_cover_binary_rename_and_hostile_paths() {
        let mut file = one_file();
        file.is_binary = true;
        assert_eq!(
            render_hunk_patch(&file, &file.hunks[0]),
            Err(RenderRefusal::Binary)
        );
        let mut file = one_file();
        file.status = FileStatus::Renamed;
        assert_eq!(
            render_hunk_patch(&file, &file.hunks[0]),
            Err(RenderRefusal::Rename)
        );
        let mut file = one_file();
        file.new_path = Some(b"evil\nname".to_vec());
        assert_eq!(
            render_hunk_patch(&file, &file.hunks[0]),
            Err(RenderRefusal::UnsafePath)
        );
    }

    #[test]
    fn no_newline_marker_survives_the_round_trip() {
        let patch = "\
diff --git a/x b/x
--- a/x
+++ b/x
@@ -1,1 +1,1 @@
-old line
+new line
\\ No newline at end of file
";
        let mut cs = parse_unified(patch.as_bytes()).changeset;
        let file = cs.files.remove(0);
        let bytes = render_hunk_patch(&file, &file.hunks[0]).unwrap();
        let round = parse_unified(&bytes);
        assert!(round.warnings.is_empty());
        assert!(
            round.changeset.files[0].hunks[0]
                .lines
                .last()
                .unwrap()
                .no_newline
        );
    }
}
