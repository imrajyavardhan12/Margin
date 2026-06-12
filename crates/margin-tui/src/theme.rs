//! Visual styles. Hardcoded default until issue #6 brings themes from
//! config; ANSI-16-safe colors only, so this renders everywhere.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub addition: Style,
    pub deletion: Style,
    pub context: Style,
    /// Background tints behind syntax-colored added/removed content.
    pub addition_tint: Style,
    pub deletion_tint: Style,
    /// Stronger backgrounds for the intra-line changed words.
    pub addition_emphasis: Style,
    pub deletion_emphasis: Style,
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
            addition_tint: Style::default().bg(Color::Rgb(0x0d, 0x33, 0x18)),
            deletion_tint: Style::default().bg(Color::Rgb(0x3d, 0x15, 0x17)),
            addition_emphasis: Style::default().bg(Color::Rgb(0x1c, 0x6b, 0x35)),
            deletion_emphasis: Style::default().bg(Color::Rgb(0x8b, 0x2d, 0x30)),
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
