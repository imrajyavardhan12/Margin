//! # margin-tui
//!
//! Rendering and interaction, structured as the Elm architecture (ADR 0003):
//!
//! ```text
//! AppState  — the single source of truth (cursor, layout, search, ...)
//! Msg       — every possible interaction, one enum
//! update()  — pure-ish: (AppState, Msg) -> AppState (+ commands)
//! view()    — pure: &AppState -> ratatui Frame
//! ```
//!
//! ## Contract (ADR 0003, ADR 0004, ADR 0010)
//!
//! - Depends on `margin-core` only — never on `margin-vcs`. Changesets are
//!   handed in; this crate does not know where they came from.
//! - `view()` has no side effects, so every screen is snapshot-testable with
//!   ratatui's `TestBackend` + insta at fixed terminal sizes.
//! - Keybindings map to `Msg` in one table (`keymap`), so user-customizable
//!   keymaps later are a data change, not a refactor.
//! - Syntax highlighting is lazy (highlight on first visibility) and never
//!   blocks the input loop — a performance budget enforced in CI (issue #6).
//!
//! Modules land with issue #4: `app`, `keymap`, `theme`, `view::{sidebar,
//! unified, side_by_side, help, picker, search}`, `highlight`.

use margin_core::Changeset;

/// Placeholder app state so the workspace compiles before issue #4.
#[derive(Debug, Default)]
pub struct AppState {
    /// The changeset under review.
    pub changeset: Changeset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appstate_constructs() {
        let state = AppState::default();
        assert!(state.changeset.is_empty());
    }
}
