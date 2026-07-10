//! The `?` overlay: the in-app cheat sheet.

use ratatui::layout::Rect;
use ratatui::text::Line as TLine;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

const KEYS: &[(&str, &str)] = &[
    ("j / k", "move down / up"),
    ("J / K", "next / previous hunk"),
    ("] / [", "next / previous file"),
    ("gg / G", "top / bottom"),
    ("Ctrl-d / Ctrl-u", "half page down / up"),
    ("/", "search (regex, smart-case)"),
    ("n / N", "next / previous match"),
    ("f", "jump to file (fuzzy)"),
    ("s / u", "stage / unstage hunk"),
    ("x", "discard hunk (typed confirm)"),
    ("r", "reload the diff"),
    ("v", "toggle unified / side-by-side"),
    ("w", "toggle line wrap"),
    ("za / zA", "collapse file / all files"),
    ("b", "toggle sidebar"),
    ("?", "toggle this help"),
    ("q", "quit"),
];

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let width = u16::min(44, area.width.saturating_sub(2));
    let height = u16::min(KEYS.len() as u16 + 4, area.height.saturating_sub(2));
    let popup = centered(area, width, height);

    let lines: Vec<TLine> = KEYS
        .iter()
        .map(|(key, action)| TLine::from(format!("  {key:<16} {action}")))
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.help_border)
        .title(" margin \u{2014} keys ")
        .padding(Padding::vertical(1));

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: u16::min(width, area.width),
        height: u16::min(height, area.height),
    }
}
