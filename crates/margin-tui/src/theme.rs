//! Visual styles. Hardcoded default until issue #6 brings themes from
//! config; ANSI-16-safe colors only, so this renders everywhere.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub addition: Style,
    pub deletion: Style,
    pub context: Style,
    pub line_no: Style,
    pub file_header: Style,
    pub hunk_header: Style,
    pub meta: Style,
    pub cursor_line: Style,
    pub sidebar_title: Style,
    pub sidebar_selected: Style,
    pub status_bar: Style,
    pub help_border: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            addition: Style::default().fg(Color::Green),
            deletion: Style::default().fg(Color::Red),
            context: Style::default(),
            line_no: Style::default().fg(Color::DarkGray),
            file_header: Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            hunk_header: Style::default().fg(Color::Cyan),
            meta: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            cursor_line: Style::default().bg(Color::DarkGray),
            sidebar_title: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            sidebar_selected: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            status_bar: Style::default().fg(Color::Black).bg(Color::Gray),
            help_border: Style::default().fg(Color::Cyan),
        }
    }
}
