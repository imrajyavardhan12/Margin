//! Enforced frame/search budgets (issue #99).
//!
//! Same contract as the parser budgets in `margin-core`: hang detectors,
//! not benchmarks. Each workload must complete with a sane, exactly
//! asserted result inside a generous wall-clock budget (60 s, ~50x the
//! slowest observed debug time). Measurement lives in the criterion
//! benches (`benches/first_frame.rs`, informational).
//!
//! Typical times on an M-series Mac: 10k-line first frame ~4 ms release
//! / ~170 ms debug; 250k-line first frame ~15 ms release / ~30 ms debug;
//! one 250k-line search keystroke ~20 ms release / ~1.3 s debug.
//!
//! The per-frame highlight budget itself is covered deterministically in
//! `highlight.rs` (`budget_exhaustion_reports_pending_and_resumes`): work
//! beyond the frame budget defers to fill-in frames instead of blocking
//! scrolling.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use margin_core::parse_unified;
use margin_tui::{render_view, update, AppState, Msg};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Hang-detector budget shared by every workload below.
const BUDGET: Duration = Duration::from_secs(60);

/// 100 files x 100 changed lines at a 200x50 viewport.
fn medium_state() -> (AppState, Vec<u8>) {
    let mut out = String::new();
    for f in 0..100 {
        out.push_str(&format!(
            "diff --git a/src/file{f}.rs b/src/file{f}.rs\n\
             index 1111111..2222222 100644\n\
             --- a/src/file{f}.rs\n\
             +++ b/src/file{f}.rs\n\
             @@ -0,0 +1,100 @@\n"
        ));
        for l in 0..100 {
            out.push_str(&format!("+let value_{l} = compute({l}) + {f};\n"));
        }
    }
    let changeset = parse_unified(out.as_bytes()).changeset;
    let mut state = AppState::new(changeset);
    update(&mut state, Msg::Resize(200, 50));
    (state, out.into_bytes())
}

/// One 250k-line file at a 200x50 viewport.
fn giant_state() -> AppState {
    let mut out =
        String::from("diff --git a/src/big.rs b/src/big.rs\nindex 1111111..2222222 100644\n--- a/src/big.rs\n+++ b/src/big.rs\n@@ -0,0 +1,250000 @@\n");
    for l in 0..250_000 {
        out.push_str(&format!("+let value_{l} = compute({l});\n"));
    }
    let changeset = parse_unified(out.as_bytes()).changeset;
    assert_eq!(changeset.additions(), 250_000);
    let mut state = AppState::new(changeset);
    update(&mut state, Msg::Resize(200, 50));
    state
}

fn draw(state: &mut AppState) {
    let mut terminal = Terminal::new(TestBackend::new(200, 50)).unwrap();
    terminal.draw(|f| render_view(state, f)).unwrap();
}

#[test]
fn first_frame_10k_lines_renders() {
    let started = Instant::now();
    let (mut state, _) = medium_state();
    draw(&mut state);
    assert!(
        started.elapsed() < BUDGET,
        "10k-line first frame took {:?}",
        started.elapsed()
    );
    assert!(!state.rows.is_empty(), "the review rendered rows");
}

#[test]
fn first_frame_250k_lines_represents_the_whole_stream() {
    let started = Instant::now();
    let mut state = giant_state();
    draw(&mut state);
    assert!(
        started.elapsed() < BUDGET,
        "250k-line first frame took {:?}",
        started.elapsed()
    );
    // The row stream holds the whole hunk (linear memory is fine); the
    // per-frame highlight budget bounds the *work*, covered by the
    // deterministic budget test in highlight.rs.
    assert!(state.rows.len() > 250_000, "giant hunk fully represented");
}

#[test]
fn scroll_250k_lines_progresses() {
    let mut state = giant_state();
    draw(&mut state);
    let started = Instant::now();
    let from = state.cursor;
    for _ in 0..20 {
        update(&mut state, Msg::HalfPageDown);
        draw(&mut state);
    }
    assert!(
        started.elapsed() < BUDGET,
        "250k-line scroll took {:?}",
        started.elapsed()
    );
    assert!(state.cursor > from, "scrolling moved through the giant");
}

#[test]
fn search_250k_lines_finds_every_occurrence() {
    let mut state = giant_state();
    draw(&mut state);
    // Setup is untimed: each keystroke rescans every row.
    update(&mut state, Msg::SearchStart);
    for c in "value_24".chars() {
        update(&mut state, Msg::SearchInput(c));
    }
    // "value_24" prefixes value_24, value_240-249, value_2400-2499,
    // value_24000-24999, and value_240000-249999: 1+10+100+1000+10000.
    assert_eq!(
        state.search.as_ref().map(|s| s.matches.len()),
        Some(11_111),
        "every occurrence found, none invented"
    );

    // The timed keystroke: backspace to "value_2", then "8".
    let started = Instant::now();
    update(&mut state, Msg::SearchBackspace);
    update(&mut state, Msg::SearchInput('8'));
    assert!(
        started.elapsed() < BUDGET,
        "250k-line search keystroke took {:?}",
        started.elapsed()
    );
    // "value_28" prefixes value_28, value_280-289, value_2800-2899, and
    // value_28000-28999 (value_280000+ is past the last line): 1111.
    assert_eq!(
        state.search.as_ref().map(|s| s.matches.len()),
        Some(1_111),
        "refined query re-matches exactly"
    );
}
