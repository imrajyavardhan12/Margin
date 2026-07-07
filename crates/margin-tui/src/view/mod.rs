//! Pure rendering: `view(&AppState, &mut Frame)` and nothing else.
//! No side effects, no state — every screen is snapshot-testable
//! (ADR-0003, ADR-0010).

mod diff;
mod help;
mod picker;
mod sidebar;
mod split;
mod style;
mod wrap;

pub(crate) use wrap::wrap_count;

/// Columns before unified content: marker (1) + line numbers (10) + sign (1).
pub(crate) const UNIFIED_PREFIX_COLS: usize = 12;

/// The marker appended to a line that lacks a trailing newline. Measurement
/// (`AppState::line_wrap_count`) and every renderer path must use this same
/// constant, or wrapped heights drift from wrapped rendering.
pub(crate) const NO_NEWLINE_SUFFIX: &str = " \u{2205}";

/// Half widths (left, right) of a split row at this main-pane width — the
/// single owner of the marker + divider arithmetic. Both the renderer
/// (`diff::split_line`) and the height math consume this, so they can
/// never disagree about geometry.
pub(crate) fn split_halves(main_width: usize) -> (usize, usize) {
    let usable = main_width.saturating_sub(2); // marker + divider
    let left = usable / 2;
    (left, usable - left)
}

/// Content columns for each half of a split row at this main-pane width.
pub(crate) fn split_content_widths(main_width: usize) -> (usize, usize) {
    let (left, right) = split_halves(main_width);
    (split::half_budget(left).1, split::half_budget(right).1)
}

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line as TLine;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;

pub fn view(state: &AppState, frame: &mut Frame) {
    // Open this frame's highlighting work budget (ADR-0006): at most a few
    // hundred lines are syntax-highlighted per frame; the rest fill in on
    // subsequent frames via the runtime's pending-work poll.
    state.highlight.begin_frame();
    let area = frame.area();
    let [content, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    // Responsive: the sidebar only earns its space on wide terminals.
    // Geometry comes from AppState::panes() so update() and view() can
    // never disagree about the main pane width.
    match state.panes().sidebar {
        Some(side_width) => {
            let [side, main] =
                Layout::horizontal([Constraint::Length(side_width), Constraint::Min(0)])
                    .areas(content);
            sidebar::render(state, frame, side);
            diff::render(state, frame, main);
        }
        None => diff::render(state, frame, content),
    }

    render_status(state, frame, status);

    if state.help_visible {
        help::render(state, frame, area);
    }
    if state.picker.is_some() {
        picker::render(state, frame, area);
    }
}

fn render_status(state: &AppState, frame: &mut Frame, area: Rect) {
    // The search bar takes over the status line while typing.
    if let Some(search) = &state.search {
        if search.typing {
            let feedback = match (&search.error, search.matches.len()) {
                (Some(err), _) => format!("  ({err})"),
                (None, 0) if !search.query.is_empty() => "  (no matches)".to_string(),
                (None, n) if n > 0 => format!("  ({n} matching rows)"),
                _ => String::new(),
            };
            let line = format!(" /{}\u{258c}{feedback}", search.query);
            frame.render_widget(
                Paragraph::new(TLine::from(line)).style(state.theme.status_bar),
                area,
            );
            return;
        }
    }

    // Command feedback (staged/refused/stale) takes the line until the
    // next keypress clears it.
    if let Some(message) = &state.status_message {
        frame.render_widget(
            Paragraph::new(TLine::from(format!(" {message}"))).style(state.theme.status_bar),
            area,
        );
        return;
    }

    let left = match state.current_file() {
        Some(idx) => {
            let path = state
                .changeset
                .files
                .get(idx)
                .map(|f| f.display_path().into_owned())
                .unwrap_or_default();
            let layout = if state.split_active { "  [split]" } else { "" };
            let wrap = if state.wrap { "  [wrap]" } else { "" };
            let search = match (&state.search, state.match_position()) {
                (Some(s), Some((pos, total))) => format!("  /{} {pos}/{total}", s.query),
                (Some(s), None) => format!("  /{} 0/{}", s.query, s.matches.len()),
                (None, _) => String::new(),
            };
            format!(
                " {path}  {}/{}{layout}{wrap}{search}",
                state.cursor + 1,
                state.rows.len()
            )
        }
        None => " no changes".to_string(),
    };
    let hints = "j/k  J/K hunk  ]/[ file  / search  f jump  v layout  ? help  q ";

    // Pad so the hints sit right-aligned when they fit; when they don't,
    // ratatui clips at the pane edge — no manual truncation, which would
    // panic on a multi-byte character boundary.
    let width = usize::from(area.width);
    let left_cols = left.chars().count();
    let mut line = left;
    if width > left_cols + hints.len() {
        line.push_str(&" ".repeat(width - left_cols - hints.len()));
        line.push_str(hints);
    }
    frame.render_widget(
        Paragraph::new(TLine::from(line)).style(state.theme.status_bar),
        area,
    );
}

/// Replace control characters so diff content can never smuggle escape
/// sequences into the terminal (see SECURITY.md), and expand tabs, which
/// ratatui renders as zero-width.
pub(crate) fn printable(content: &[u8]) -> String {
    String::from_utf8_lossy(content)
        .chars()
        .flat_map(|c| match c {
            '\t' => "    ".chars().collect::<Vec<_>>(),
            c if c.is_control() => vec!['\u{fffd}'],
            c => vec![c],
        })
        .collect()
}
