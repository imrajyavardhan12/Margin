//! Pure rendering: `view(&AppState, &mut Frame)` and nothing else.
//! No side effects, no state — every screen is snapshot-testable
//! (ADR-0003, ADR-0010).

mod diff;
mod help;
mod sidebar;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line as TLine;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;

pub fn view(state: &AppState, frame: &mut Frame) {
    let area = frame.area();
    let [content, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    // Responsive: the sidebar only earns its space on wide terminals.
    let show_sidebar = state.sidebar_visible && content.width >= 60;
    if show_sidebar {
        let side_width = u16::min(32, content.width / 3);
        let [side, main] =
            Layout::horizontal([Constraint::Length(side_width), Constraint::Min(0)]).areas(content);
        sidebar::render(state, frame, side);
        diff::render(state, frame, main);
    } else {
        diff::render(state, frame, content);
    }

    render_status(state, frame, status);

    if state.help_visible {
        help::render(state, frame, area);
    }
}

fn render_status(state: &AppState, frame: &mut Frame, area: Rect) {
    let left = match state.current_file() {
        Some(idx) => {
            let path = state
                .changeset
                .files
                .get(idx)
                .map(|f| f.display_path().into_owned())
                .unwrap_or_default();
            format!(" {path}  {}/{}", state.cursor + 1, state.rows.len())
        }
        None => " no changes".to_string(),
    };
    let hints = "j/k move  J/K hunk  ]/[ file  b sidebar  ? help  q quit ";

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
