//! Stage and unstage reviewed hunks — the index-only write path
//! (ADR-0013, issue #10).
//!
//! This module is deliberately dumb: it applies whatever single-hunk
//! patch bytes margin-core rendered (`render_hunk_patch` to stage,
//! `render_reversed_hunk_patch` to unstage). `ApplyLocation::Index`
//! means libgit2 structurally cannot touch the working tree, and the
//! dry-run pass means a stale hunk leaves the index exactly as it was.

use std::path::Path;

use git2::{ApplyLocation, ApplyOptions, Diff, Repository};

/// Why an index apply didn't happen.
#[derive(Debug, thiserror::Error)]
pub enum StageError {
    /// The hunk no longer matches the index content — the world moved
    /// since the review loaded (or it was already staged). The index
    /// was not modified; the caller should reload.
    #[error("hunk no longer applies; reload to review the current state")]
    Stale(#[source] git2::Error),
    /// Everything else (not a repository, corrupt patch, io).
    #[error(transparent)]
    Git(#[from] git2::Error),
}

/// Apply single-hunk patch bytes to the index only. Stage = forward
/// patch, unstage = reversed patch; the caller chooses by rendering.
pub fn apply_patch_to_index(repo_path: &Path, patch: &[u8]) -> Result<(), StageError> {
    let repo = Repository::discover(repo_path)?;
    let diff = Diff::from_buffer(patch)?;
    // Dry-run first (ADR-0013): validate the whole hunk applies before
    // writing anything, so failure can never half-apply.
    let mut check = ApplyOptions::new();
    check.check(true);
    repo.apply(&diff, ApplyLocation::Index, Some(&mut check))
        .map_err(StageError::Stale)?;
    repo.apply(&diff, ApplyLocation::Index, None)?;
    Ok(())
}
