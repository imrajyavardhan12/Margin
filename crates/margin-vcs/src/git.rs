//! git2-backed [`DiffSource`] implementations.
//!
//! All git2 types stay inside this module (ADR-0005); everything crossing
//! the boundary is translated into `margin-core` types. Conversion is
//! structural (delta -> Patch -> hunks -> lines) rather than
//! format-and-reparse, for fidelity and speed.

use std::path::{Path, PathBuf};

use git2::{Delta, Diff, DiffFindOptions, DiffLineType, DiffOptions, Repository, Tree};
use margin_core::{Changeset, FileDiff, FileStatus, Hunk, Line, LineKind};

use crate::{DiffId, DiffSource, SourceError};

/// Working tree (and index) vs HEAD — what `margin` shows by default.
/// Untracked files are included as additions: agents create new files
/// constantly and `git diff` silently hiding them is a footgun.
pub struct GitWorktree {
    pub repo_path: PathBuf,
    pub include_untracked: bool,
}

impl GitWorktree {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            include_untracked: true,
        }
    }
}

impl DiffSource for GitWorktree {
    fn load(&self) -> Result<Changeset, SourceError> {
        let repo = open_repo(&self.repo_path)?;
        let head = head_tree(&repo)?;
        let mut opts = base_options();
        if self.include_untracked {
            opts.include_untracked(true)
                .recurse_untracked_dirs(true)
                .show_untracked_content(true);
        }
        let mut diff = repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut opts))?;
        detect_renames(&mut diff)?;
        convert(&diff)
    }

    fn id(&self) -> DiffId {
        DiffId(format!("{}#worktree", self.repo_path.display()))
    }
}

/// Index vs HEAD — `margin diff --staged`.
pub struct GitStaged {
    pub repo_path: PathBuf,
}

impl GitStaged {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }
}

impl DiffSource for GitStaged {
    fn load(&self) -> Result<Changeset, SourceError> {
        let repo = open_repo(&self.repo_path)?;
        let head = head_tree(&repo)?;
        let mut opts = base_options();
        let mut diff = repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts))?;
        detect_renames(&mut diff)?;
        convert(&diff)
    }

    fn id(&self) -> DiffId {
        DiffId(format!("{}#staged", self.repo_path.display()))
    }
}

/// One commit vs its first parent — `margin show [rev]`.
/// Root commits diff against the empty tree.
pub struct GitShow {
    pub repo_path: PathBuf,
    pub spec: String,
}

impl GitShow {
    pub fn new(repo_path: impl Into<PathBuf>, spec: impl Into<String>) -> Self {
        Self {
            repo_path: repo_path.into(),
            spec: spec.into(),
        }
    }
}

impl DiffSource for GitShow {
    fn load(&self) -> Result<Changeset, SourceError> {
        let repo = open_repo(&self.repo_path)?;
        let commit = repo
            .revparse_single(&self.spec)
            .and_then(|obj| obj.peel_to_commit())
            .map_err(|e| SourceError::BadRevspec {
                spec: self.spec.clone(),
                reason: e.message().to_string(),
            })?;
        let new_tree = commit.tree()?;
        let old_tree = match commit.parent(0) {
            Ok(parent) => Some(parent.tree()?),
            Err(_) => None, // root commit
        };
        let mut opts = base_options();
        let mut diff =
            repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
        detect_renames(&mut diff)?;
        convert(&diff)
    }

    fn id(&self) -> DiffId {
        DiffId(format!("{}#show:{}", self.repo_path.display(), self.spec))
    }
}

/// Two revisions — `margin diff A..B`. Each side may be any tree-ish.
pub struct GitRevRange {
    pub repo_path: PathBuf,
    pub from: String,
    pub to: String,
}

impl GitRevRange {
    pub fn new(
        repo_path: impl Into<PathBuf>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            repo_path: repo_path.into(),
            from: from.into(),
            to: to.into(),
        }
    }
}

impl DiffSource for GitRevRange {
    fn load(&self) -> Result<Changeset, SourceError> {
        let repo = open_repo(&self.repo_path)?;
        let old_tree = resolve_tree(&repo, &self.from)?;
        let new_tree = resolve_tree(&repo, &self.to)?;
        let mut opts = base_options();
        let mut diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut opts))?;
        detect_renames(&mut diff)?;
        convert(&diff)
    }

    fn id(&self) -> DiffId {
        DiffId(format!(
            "{}#range:{}..{}",
            self.repo_path.display(),
            self.from,
            self.to
        ))
    }
}

fn open_repo(path: &Path) -> Result<Repository, SourceError> {
    Repository::discover(path).map_err(|_| SourceError::NotARepository {
        path: path.to_path_buf(),
    })
}

/// HEAD's tree, or `None` on an unborn branch (fresh `git init`).
fn head_tree(repo: &Repository) -> Result<Option<Tree<'_>>, SourceError> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_tree()?)),
        Err(_) => Ok(None),
    }
}

fn resolve_tree<'r>(repo: &'r Repository, spec: &str) -> Result<Tree<'r>, SourceError> {
    repo.revparse_single(spec)
        .and_then(|obj| obj.peel_to_tree())
        .map_err(|e| SourceError::BadRevspec {
            spec: spec.to_string(),
            reason: e.message().to_string(),
        })
}

fn base_options() -> DiffOptions {
    let mut opts = DiffOptions::new();
    opts.include_typechange(true);
    opts
}

fn detect_renames(diff: &mut Diff<'_>) -> Result<(), SourceError> {
    let mut find = DiffFindOptions::new();
    find.renames(true).copies(true);
    diff.find_similar(Some(&mut find))?;
    Ok(())
}

/// Translate a git2 diff into the margin-core model.
fn convert(diff: &Diff<'_>) -> Result<Changeset, SourceError> {
    let mut files = Vec::new();
    for (idx, delta) in diff.deltas().enumerate() {
        let status = match delta.status() {
            Delta::Unmodified | Delta::Ignored => continue,
            Delta::Added | Delta::Untracked => FileStatus::Added,
            Delta::Deleted => FileStatus::Deleted,
            Delta::Renamed => FileStatus::Renamed,
            Delta::Copied => FileStatus::Copied,
            Delta::Modified | Delta::Typechange | Delta::Conflicted | Delta::Unreadable => {
                FileStatus::Modified
            }
        };

        let mut file = FileDiff {
            status,
            is_binary: delta.flags().is_binary(),
            ..FileDiff::default()
        };
        if status != FileStatus::Added {
            file.old_path = delta.old_file().path_bytes().map(<[u8]>::to_vec);
            file.old_mode = file_mode(delta.old_file().mode());
        }
        if status != FileStatus::Deleted {
            file.new_path = delta.new_file().path_bytes().map(<[u8]>::to_vec);
            file.new_mode = file_mode(delta.new_file().mode());
        }

        if let Some(patch) = git2::Patch::from_diff(diff, idx)? {
            file.is_binary = patch.delta().flags().is_binary() || file.is_binary;
            for h in 0..patch.num_hunks() {
                let (header, line_count) = patch.hunk(h)?;
                let mut hunk = Hunk {
                    old_start: header.old_start(),
                    old_count: header.old_lines(),
                    new_start: header.new_start(),
                    new_count: header.new_lines(),
                    heading: heading_from_header(header.header()),
                    lines: Vec::with_capacity(line_count),
                };
                for l in 0..line_count {
                    let line = patch.line_in_hunk(h, l)?;
                    let kind = match line.origin_value() {
                        DiffLineType::Context => LineKind::Context,
                        DiffLineType::Addition => LineKind::Addition,
                        DiffLineType::Deletion => LineKind::Deletion,
                        DiffLineType::ContextEOFNL
                        | DiffLineType::AddEOFNL
                        | DiffLineType::DeleteEOFNL => {
                            if let Some(last) = hunk.lines.last_mut() {
                                last.no_newline = true;
                            }
                            continue;
                        }
                        // File/hunk header pseudo-lines never occur inside
                        // line_in_hunk, but stay total over the enum.
                        _ => continue,
                    };
                    let content = line.content();
                    let content = content.strip_suffix(b"\n").unwrap_or(content).to_vec();
                    hunk.lines.push(Line {
                        kind,
                        content,
                        no_newline: false,
                    });
                }
                file.hunks.push(hunk);
            }
        }
        files.push(file);
    }
    Ok(Changeset { files })
}

/// Octal-style mode as a u32 (e.g. 0o100644); `None` for absent sides.
fn file_mode(mode: git2::FileMode) -> Option<u32> {
    match mode {
        git2::FileMode::Unreadable => None,
        other => Some(i32::from(other) as u32),
    }
}

/// Extract the section heading from a raw `@@ ... @@ heading` header.
fn heading_from_header(header: &[u8]) -> Option<Vec<u8>> {
    let after_open = header.strip_prefix(b"@@")?;
    let close = after_open.windows(2).position(|w| w == b"@@")?;
    let tail = after_open.get(close + 2..)?;
    let tail = tail.strip_suffix(b"\n").unwrap_or(tail);
    let tail = tail.strip_suffix(b"\r").unwrap_or(tail);
    let tail = tail.strip_prefix(b" ").unwrap_or(tail);
    if tail.is_empty() {
        None
    } else {
        Some(tail.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::heading_from_header;

    #[test]
    fn heading_extraction() {
        assert_eq!(
            heading_from_header(b"@@ -1,5 +1,6 @@ fn main()\n"),
            Some(b"fn main()".to_vec())
        );
        assert_eq!(heading_from_header(b"@@ -1,5 +1,6 @@\n"), None);
        assert_eq!(heading_from_header(b"not a header"), None);
    }
}
