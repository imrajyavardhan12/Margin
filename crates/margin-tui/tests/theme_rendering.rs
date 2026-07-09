//! Theme rendering invariants (issue #6). Frame *symbols* are identical
//! across themes — snapshots can't see color — so these tests assert on
//! cell styles: each truecolor theme leaves its fingerprint, the 16-color
//! palette never emits RGB, and monochrome emits no color at all.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use margin_core::parse_unified;
use margin_tui::theme::{ColorMode, Theme, THEME_NAMES};
use margin_tui::{render_view, update, AppState, Msg, StagedFiles};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

const SAMPLE: &str = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,3 +1,3 @@ fn setup()
 use std::env;
-fn old() {}
+fn new_one() {}
";

fn rendered_styles(theme: Theme) -> Vec<ratatui::style::Style> {
    let mut state = AppState::new(parse_unified(SAMPLE.as_bytes()).changeset);
    state.apply_theme(theme);
    update(&mut state, Msg::Resize(80, 24));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    // Draw twice: the second frame has the lazy highlight cache warm.
    terminal.draw(|f| render_view(&state, f)).unwrap();
    terminal.draw(|f| render_view(&state, f)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let area = buffer.area();
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|(x, y)| buffer[(x, y)].style())
        .collect()
}

fn is_rgb(color: Color) -> bool {
    matches!(color, Color::Rgb(..))
}

#[test]
fn truecolor_themes_paint_their_tints_and_differ() {
    let mut fingerprints = Vec::new();
    for name in THEME_NAMES {
        let theme = Theme::resolve(name, ColorMode::TrueColor).unwrap();
        let tint = theme.addition_tint.bg;
        let styles = rendered_styles(theme);
        if let Some(expected) = tint {
            assert!(
                styles.iter().any(|s| s.bg == Some(expected)),
                "{name}: addition tint must appear on the added line"
            );
        }
        fingerprints.push(format!("{styles:?}"));
    }
    for i in 1..fingerprints.len() {
        assert_ne!(
            fingerprints[0], fingerprints[i],
            "themes must be visually distinct ({} vs {})",
            THEME_NAMES[0], THEME_NAMES[i]
        );
    }
}

#[test]
fn ansi16_never_emits_rgb() {
    let theme = Theme::resolve("ledger", ColorMode::Ansi16).unwrap();
    assert_eq!(theme.syntax_theme, None);
    for style in rendered_styles(theme) {
        assert!(
            !style.fg.is_some_and(is_rgb) && !style.bg.is_some_and(is_rgb),
            "16-color mode painted RGB: {style:?}"
        );
    }
}

#[test]
fn monochrome_emits_no_color_at_all() {
    let theme = Theme::resolve("carbon", ColorMode::Monochrome).unwrap();
    for style in rendered_styles(theme) {
        for color in [style.fg, style.bg].into_iter().flatten() {
            assert_eq!(color, Color::Reset, "NO_COLOR mode painted {color:?}");
        }
    }
}

/// The sidebar's staged dot is the one cell painted in `sidebar_staged`;
/// snapshots can't see its color, so assert on the cell style directly.
#[test]
fn staged_indicator_wears_the_staged_style() {
    let mut state = AppState::new(parse_unified(SAMPLE.as_bytes()).changeset);
    let theme = Theme::resolve("ledger", ColorMode::TrueColor).unwrap();
    let staged_fg = theme.sidebar_staged.fg;
    state.apply_theme(theme);
    // Stage the sample's only file, so its sidebar row lights up.
    state.staged = Some(StagedFiles::from_staged_changeset(
        &parse_unified(SAMPLE.as_bytes()).changeset,
    ));
    update(&mut state, Msg::Resize(80, 24));

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| render_view(&state, f)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let area = buffer.area();
    let marked = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            let cell = &buffer[(x, y)];
            cell.symbol() == "\u{25cf}" && cell.style().fg == staged_fg
        });
    assert!(marked, "the staged dot must be painted in sidebar_staged");
}
