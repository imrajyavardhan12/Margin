//! # margin-core
//!
//! The pure heart of Margin: the changeset data model, the unified-diff
//! parser, and intra-line (word-level) diffing.
//!
//! ## Contract (ADR 0003, ADR 0004)
//!
//! - **No I/O.** This crate never touches the filesystem, network, or a git
//!   repository. It transforms bytes and data structures, nothing else.
//! - **No TUI dependencies.** Rendering lives in `margin-tui`.
//! - **Panic-free on untrusted input** (ADR 0009). The parser is fuzzed;
//!   malformed patches produce errors, never panics.
//!
//! Because of this contract, everything here is unit-testable, fuzzable, and
//! reusable — `margin --json` is just a serialization of [`Changeset`].
//!
//! ## Model hierarchy (built in issue #2)
//!
//! ```text
//! Changeset            // one review session's worth of changes
//! └── FileDiff         // old/new path, status (added/deleted/renamed/...), mode
//!     └── Hunk         // @@ header, old/new line ranges
//!         └── Line     // context / addition / deletion, with intra-line spans
//! ```

/// Placeholder so the workspace compiles before issue #2 lands.
/// Replaced by the real `Changeset` model.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Changeset {
    /// Number of files in the changeset.
    pub files: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_changeset_is_empty() {
        assert_eq!(Changeset::default().files, 0);
    }
}
