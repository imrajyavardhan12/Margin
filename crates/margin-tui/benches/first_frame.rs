//! Frame-time benchmarks (ADR-0010). The blueprint budgets, measured in
//! release mode on the 100-file/10k-line synthetic diff:
//!
//! - `first_frame`: parse + AppState + first render at 200x50 — budget 50 ms
//! - `scroll_frame`: a warm-cache scroll redraw — must stay comfortably
//!   inside a 60 fps frame (16 ms), highlighting bounded by the per-frame
//!   budget regardless of diff size

#![allow(clippy::unwrap_used, clippy::expect_used)]

use criterion::{criterion_group, criterion_main, Criterion};
use margin_core::parse_unified;
use margin_tui::{render_view, update, AppState, Msg};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn synthetic_patch(files: usize, lines_per_file: usize) -> Vec<u8> {
    let mut out = String::new();
    for f in 0..files {
        out.push_str(&format!(
            "diff --git a/src/file{f}.rs b/src/file{f}.rs\n\
             index 1111111..2222222 100644\n\
             --- a/src/file{f}.rs\n\
             +++ b/src/file{f}.rs\n\
             @@ -0,0 +1,{lines_per_file} @@\n"
        ));
        for l in 0..lines_per_file {
            out.push_str(&format!("+let value_{l} = compute({l}) + {f};\n"));
        }
    }
    out.into_bytes()
}

fn benches(c: &mut Criterion) {
    let patch = synthetic_patch(100, 100);

    c.bench_function("first_frame/100_files_10k_lines_200x50", |b| {
        b.iter(|| {
            let mut state = AppState::new(parse_unified(&patch).changeset);
            update(&mut state, Msg::Resize(200, 50));
            let mut terminal = Terminal::new(TestBackend::new(200, 50)).unwrap();
            terminal.draw(|f| render_view(&state, f)).unwrap();
        })
    });

    c.bench_function("scroll_frame/warm_cache", |b| {
        let mut state = AppState::new(parse_unified(&patch).changeset);
        update(&mut state, Msg::Resize(200, 50));
        let mut terminal = Terminal::new(TestBackend::new(200, 50)).unwrap();
        terminal.draw(|f| render_view(&state, f)).unwrap();
        b.iter(|| {
            update(&mut state, Msg::HalfPageDown);
            terminal.draw(|f| render_view(&state, f)).unwrap();
        })
    });

    // AC (issue #7): a search keystroke over the giant diff stays under
    // 100 ms — each input recompiles the regex and rescans every row.
    c.bench_function("search_keystroke/250k_lines", |b| {
        let giant = synthetic_patch(1, 250_000);
        let mut state = AppState::new(parse_unified(&giant).changeset);
        update(&mut state, Msg::Resize(200, 50));
        update(&mut state, Msg::SearchStart);
        for c in "value_24".chars() {
            update(&mut state, Msg::SearchInput(c));
        }
        b.iter(|| {
            update(&mut state, Msg::SearchBackspace);
            update(&mut state, Msg::SearchInput('8'));
        })
    });

    // The pathological case: one 250k-line hunk. The first frame must stay
    // bounded by the highlight budget, not the hunk size.
    let giant = synthetic_patch(1, 250_000);
    c.bench_function("first_frame/250k_line_file_200x50", |b| {
        b.iter(|| {
            let mut state = AppState::new(parse_unified(&giant).changeset);
            update(&mut state, Msg::Resize(200, 50));
            let mut terminal = Terminal::new(TestBackend::new(200, 50)).unwrap();
            terminal.draw(|f| render_view(&state, f)).unwrap();
        })
    });
}

criterion_group!(frames, benches);
criterion_main!(frames);
