//! # margin-tui
//!
//! Rendering and interaction, structured as the Elm architecture (ADR-0003):
//!
//! ```text
//! AppState  — the single source of truth (cursor, layout, help, ...)
//! Msg       — every possible interaction, one enum
//! update()  — the only place state changes
//! view()    — pure: &AppState -> frame; no side effects, snapshot-testable
//! ```
//!
//! ## Contract (ADR-0003, ADR-0004, ADR-0010)
//!
//! - Depends on `margin-core` only — never on `margin-vcs`. Changesets are
//!   handed in; this crate does not know where they came from.
//! - `view()` has no side effects, so every screen is snapshot-testable with
//!   ratatui's `TestBackend` + insta at fixed terminal sizes.
//! - Keybindings map to `Msg` in one table (`keymap`), so user-customizable
//!   keymaps later are a data change, not a refactor.
//! - The only effectful module is [`runtime`]: terminal setup/teardown, the
//!   event loop, and the panic guard that restores the terminal (ADR-0009).
//!
//! Coming next: side-by-side layout (issue #3), lazy syntax highlighting and
//! intra-line emphasis (issue #4), themes from config (issue #6), search and
//! the fuzzy file picker (issue #7).

pub mod app;
pub mod keymap;
mod runtime;
pub mod theme;
pub mod view;

pub use app::{update, AppState, Msg};
pub use runtime::run;
pub use view::view as render_view;
