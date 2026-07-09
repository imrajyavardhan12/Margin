//! Discard a reviewed hunk from the working tree — Margin's only
//! destructive operation (ADR-0014, issue #11).
//!
//! The safety order is the whole point: the forward patch (what the
//! reviewer saw, `render_hunk_patch` bytes) is persisted to the trash
//! **before** the reversed patch touches the working tree, and the apply
//! runs a dry-run first, exactly like staging (ADR-0013). Trash entries
//! are plain patches: even without Margin,
//! `git apply .git/margin/trash/<file>.patch` restores.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use git2::{ApplyLocation, ApplyOptions, Diff, Repository};

use crate::staging::StageError;

/// Why an undo didn't happen.
#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    /// The trash is empty — nothing to restore.
    #[error("nothing to undo (the discard trash is empty)")]
    Empty,
    /// The newest trash entry no longer applies: the working tree moved
    /// since the discard. The entry is kept for hand-recovery.
    #[error(
        "the discarded hunk no longer applies cleanly; restore by hand from {}",
        path.display()
    )]
    Stale { path: PathBuf },
    #[error("cannot access the discard trash: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
}

/// Apply reversed single-hunk patch bytes to the working tree only.
/// Same discipline as `apply_patch_to_index`: dry-run first, so a stale
/// hunk fails cleanly with the tree untouched (`StageError::Stale`).
/// The caller is responsible for writing the trash entry *first*
/// (ADR-0014: nothing is destroyed before a copy exists).
pub fn apply_patch_to_worktree(repo_path: &Path, patch: &[u8]) -> Result<(), StageError> {
    let repo = Repository::discover(repo_path)?;
    let diff = Diff::from_buffer(patch)?;
    let mut check = ApplyOptions::new();
    check.check(true);
    repo.apply(&diff, ApplyLocation::WorkDir, Some(&mut check))
        .map_err(StageError::Stale)?;
    repo.apply(&diff, ApplyLocation::WorkDir, None)?;
    Ok(())
}

/// Persist a forward patch to `.git/margin/trash/<millis>.patch` and
/// return the path. Names are zero-padded epoch milliseconds (13 digits
/// until the year 2286), so lexical order **is** discard order — the
/// invariant `undo` relies on. A same-millisecond collision bumps the
/// timestamp until free rather than suffixing: any suffix scheme breaks
/// the ordering (`-1` sorts before `.patch`) and would make undo restore
/// the wrong entry.
pub fn write_trash(repo_path: &Path, patch: &[u8]) -> Result<PathBuf, UndoError> {
    let dir = trash_dir(repo_path)?;
    std::fs::create_dir_all(&dir)?;
    let mut millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut path = dir.join(format!("{millis:013}.patch"));
    while path.exists() {
        millis += 1;
        path = dir.join(format!("{millis:013}.patch"));
    }
    std::fs::write(&path, patch)?;
    Ok(path)
}

/// Restore the newest trash entry to the working tree and delete it on
/// success. A stale entry (tree moved since the discard) is kept and
/// reported with its path for hand-recovery.
pub fn undo_last_discard(repo_path: &Path) -> Result<PathBuf, UndoError> {
    let Some(path) = latest_trash(repo_path)? else {
        return Err(UndoError::Empty);
    };
    let patch = std::fs::read(&path)?;
    let repo = Repository::discover(repo_path)?;
    let diff = Diff::from_buffer(&patch)?;
    let mut check = ApplyOptions::new();
    check.check(true);
    if repo
        .apply(&diff, ApplyLocation::WorkDir, Some(&mut check))
        .is_err()
    {
        return Err(UndoError::Stale { path });
    }
    repo.apply(&diff, ApplyLocation::WorkDir, None)?;
    std::fs::remove_file(&path)?;
    Ok(path)
}

/// The newest trash entry, if any (lexically greatest `.patch` name —
/// names are zero-padded epoch milliseconds, so lexical == newest).
fn latest_trash(repo_path: &Path) -> Result<Option<PathBuf>, UndoError> {
    let dir = trash_dir(repo_path)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut newest: Option<PathBuf> = None;
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "patch")
            && newest.as_ref().is_none_or(|best| path > *best)
        {
            newest = Some(path);
        }
    }
    Ok(newest)
}

/// `<gitdir>/margin/trash` — under the gitdir, so linked worktrees get
/// their own trash and nothing pollutes the checkout.
fn trash_dir(repo_path: &Path) -> Result<PathBuf, UndoError> {
    let repo = Repository::discover(repo_path)?;
    Ok(repo.path().join("margin/trash"))
}
