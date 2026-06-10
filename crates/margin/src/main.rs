//! The `margin` binary: CLI parsing, config discovery, source selection,
//! and the terminal session.
//!
//! Responsibilities (and nothing more — ADR 0004):
//! 1. Parse CLI args (clap) and config (ADR 0007, ADR 0008).
//! 2. Choose a `margin_vcs::DiffSource` from the invocation.
//! 3. If stdout is not a TTY in pager mode, pass input through unchanged
//!    (the "safe as core.pager" guarantee, ADR 0007).
//! 4. Otherwise run the `margin-tui` event loop.

fn main() {
    // Real CLI lands with issue #4. The scaffold proves the wiring:
    let state = margin_tui::AppState::default();
    println!(
        "margin {} — scaffold build. {} files loaded. See ROADMAP.md.",
        env!("CARGO_PKG_VERSION"),
        state.changeset.files
    );
}
