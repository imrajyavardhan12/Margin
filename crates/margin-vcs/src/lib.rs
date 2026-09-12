//! # margin-vcs
//!
//! Every way a changeset can enter Margin, behind one trait — plus every
//! way a review can write back, behind one transaction each.
//!
//! ## Contract (ADR 0004, ADR 0005, ADR 0019)
//!
//! [`DiffSource`] is the read-only changeset Interface: the TUI never
//! talks to git, the filesystem, or stdin directly — it asks a source
//! for a [`Changeset`] and renders it. Writes are separate explicit
//! seams in this same crate (ADR-0019): index writes
//! ([`apply_patch_to_index`]), the recoverable discard transaction
//! ([`discard_hunk`]), and trash undo ([`undo_last_discard`]).
//! Consequences:
//!
//! - New inputs (Jujutsu, watch mode) are new `DiffSource` impls, not new
//!   code paths through the app.
//! - Tests inject synthetic sources; the TUI is testable without a repo.
//! - git2 stays quarantined in this crate (ADR 0005): no git2 types appear
//!   in any public signature, so a future migration to gitoxide touches one
//!   module.
//!
//! Implemented inputs: [`GitWorktree`], [`GitStaged`], [`GitShow`],
//! [`GitRevRange`], [`TwoFiles`], [`GhPr`]. Patch/stdin/pager reviews
//! carry a snapshot [`Changeset`] with no source — session-only by design.

mod discard;
mod files;
mod gh;
mod git;
mod staging;

pub use discard::{
    apply_patch_to_worktree, discard_hunk, undo_last_discard, write_trash, DiscardError,
    DiscardMode, DiscardOutcome, UndoError,
};
pub use files::TwoFiles;
pub use gh::GhPr;
pub use git::{staged_paths, workdir_root, GitRevRange, GitShow, GitStaged, GitWorktree};
pub use staging::{apply_patch_to_index, StageError};

use margin_core::Changeset;
use std::path::PathBuf;

/// Stable identity of a diff across reloads, used to key persisted review
/// state (viewed files, cursor position).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiffId(pub String);

/// A producer of changesets. See crate docs for the architectural contract.
pub trait DiffSource {
    /// Load (or reload) the changeset. Called on startup and on `r`/watch.
    fn load(&self) -> Result<Changeset, SourceError>;

    /// Stable identity for persisting review state across runs.
    fn id(&self) -> DiffId;
}

/// Errors surfaced to the user as messages, never as panics (ADR-0009).
/// Typed so the binary can map them to exit codes and the TUI can decide
/// between "show message" and "give up" (ADR-0007).
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("not a git repository (or any parent): {}", path.display())]
    NotARepository { path: PathBuf },

    #[error("cannot resolve revision '{spec}': {reason}")]
    BadRevspec { spec: String, reason: String },

    #[error("cannot read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },

    #[error("git: {0}")]
    Git(String),
}

impl From<git2::Error> for SourceError {
    fn from(err: git2::Error) -> Self {
        SourceError::Git(err.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty;

    impl DiffSource for Empty {
        fn load(&self) -> Result<Changeset, SourceError> {
            Ok(Changeset::default())
        }

        fn id(&self) -> DiffId {
            DiffId("empty".into())
        }
    }

    #[test]
    fn synthetic_source_loads() {
        let source = Empty;
        let changeset = match source.load() {
            Ok(changeset) => changeset,
            Err(err) => panic!("synthetic source failed: {err}"),
        };
        assert!(changeset.is_empty());
        assert_eq!(source.id(), DiffId("empty".into()));
    }
}
