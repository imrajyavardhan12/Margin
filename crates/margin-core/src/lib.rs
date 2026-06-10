//! # margin-core
//!
//! The pure heart of Margin: the changeset data model, the unified-diff
//! parser, and (coming) intra-line diffing.
//!
//! ## Contract (ADR 0003, ADR 0004)
//!
//! - **No I/O.** This crate never touches the filesystem, network, or a git
//!   repository. It transforms bytes and data structures, nothing else.
//! - **No TUI dependencies.** Rendering lives in `margin-tui`.
//! - **Panic-free on untrusted input** (ADR 0009). The parser is tolerant by
//!   design — see [`patch`] module docs — and malformed input produces
//!   warnings, never panics.
//!
//! Because of this contract, everything here is unit-testable, fuzzable, and
//! reusable — `margin --json` is just a serialization of [`Changeset`].
//!
//! ## Model hierarchy
//!
//! ```text
//! Changeset            // one review session's worth of changes
//! └── FileDiff         // old/new path, status (added/deleted/renamed/...), mode
//!     └── Hunk         // @@ header, old/new line ranges
//!         └── Line     // context / addition / deletion (+ intra-line spans, later)
//! ```

pub mod model;
pub mod patch;

pub use model::{ByteStr, Changeset, FileDiff, FileStatus, Hunk, Line, LineKind};
pub use patch::{parse_unified, ParseOutcome, ParseWarning};
