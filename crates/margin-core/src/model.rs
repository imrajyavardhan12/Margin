//! The changeset data model: what every `DiffSource` produces and every view
//! renders.
//!
//! Content is stored as **raw bytes** (`ByteStr`), not `String`: patches and
//! file contents are not guaranteed to be valid UTF-8, and Margin must never
//! corrupt what it displays — or, from v0.2, what it applies back to the
//! index. Lossy conversion happens only at display time via the `*_lossy`
//! helpers.

use std::borrow::Cow;

/// Raw bytes from a patch (paths, line content, hunk headings).
pub type ByteStr = Vec<u8>;

/// One review session's worth of changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changeset {
    pub files: Vec<FileDiff>,
}

impl Changeset {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Total added lines across all files.
    pub fn additions(&self) -> usize {
        self.files.iter().map(FileDiff::additions).sum()
    }

    /// Total deleted lines across all files.
    pub fn deletions(&self) -> usize {
        self.files.iter().map(FileDiff::deletions).sum()
    }
}

/// How a file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

/// One file's diff: paths, status, modes, and hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path on the old side. `None` for added files.
    pub old_path: Option<ByteStr>,
    /// Path on the new side. `None` for deleted files.
    pub new_path: Option<ByteStr>,
    pub status: FileStatus,
    /// Old file mode as parsed from octal, e.g. `0o100644`.
    pub old_mode: Option<u32>,
    /// New file mode as parsed from octal, e.g. `0o100755`.
    pub new_mode: Option<u32>,
    /// `similarity index N%` for renames and copies.
    pub similarity: Option<u8>,
    /// Binary file change; `hunks` is empty.
    pub is_binary: bool,
    pub hunks: Vec<Hunk>,
}

impl Default for FileDiff {
    fn default() -> Self {
        Self {
            old_path: None,
            new_path: None,
            status: FileStatus::Modified,
            old_mode: None,
            new_mode: None,
            similarity: None,
            is_binary: false,
            hunks: Vec::new(),
        }
    }
}

impl FileDiff {
    /// The path to show in a file list: the new path, falling back to the old
    /// one (deleted files), converted lossily for display.
    pub fn display_path(&self) -> Cow<'_, str> {
        let path = self
            .new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .unwrap_or(b"<unknown>");
        String::from_utf8_lossy(path)
    }

    pub fn additions(&self) -> usize {
        self.hunks.iter().map(Hunk::additions).sum()
    }

    pub fn deletions(&self) -> usize {
        self.hunks.iter().map(Hunk::deletions).sum()
    }
}

/// One `@@ -old_start,old_count +new_start,new_count @@` block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    /// The section heading git appends after the second `@@`
    /// (usually the enclosing function signature).
    pub heading: Option<ByteStr>,
    pub lines: Vec<Line>,
}

impl Hunk {
    pub fn additions(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == LineKind::Addition)
            .count()
    }

    pub fn deletions(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == LineKind::Deletion)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Addition,
    Deletion,
}

/// One line inside a hunk, without its leading `+`/`-`/space marker and
/// without the trailing newline. Carriage returns from CRLF files are
/// preserved in `content` — they are part of the file's bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub content: ByteStr,
    /// True when followed by `\ No newline at end of file`.
    pub no_newline: bool,
}

impl Line {
    pub fn content_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_path_prefers_new_side_and_is_lossy() {
        let file = FileDiff {
            old_path: Some(b"old.rs".to_vec()),
            new_path: Some(b"new \xff.rs".to_vec()),
            ..FileDiff::default()
        };
        assert_eq!(file.display_path(), "new \u{fffd}.rs");

        let deleted = FileDiff {
            old_path: Some(b"gone.rs".to_vec()),
            ..FileDiff::default()
        };
        assert_eq!(deleted.display_path(), "gone.rs");
    }

    #[test]
    fn counts_roll_up_from_lines() {
        let hunk = Hunk {
            lines: vec![
                Line {
                    kind: LineKind::Context,
                    content: b"ctx".to_vec(),
                    no_newline: false,
                },
                Line {
                    kind: LineKind::Addition,
                    content: b"add".to_vec(),
                    no_newline: false,
                },
                Line {
                    kind: LineKind::Deletion,
                    content: b"del".to_vec(),
                    no_newline: false,
                },
            ],
            ..Hunk::default()
        };
        let changeset = Changeset {
            files: vec![FileDiff {
                hunks: vec![hunk],
                ..FileDiff::default()
            }],
        };
        assert_eq!(changeset.additions(), 1);
        assert_eq!(changeset.deletions(), 1);
    }
}
