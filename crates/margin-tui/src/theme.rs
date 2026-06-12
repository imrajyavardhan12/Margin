//! Themes and color-capability handling (ADR-0008, issue #6).
//!
//! Four built-in truecolor themes, selected by name; below truecolor the
//! palette degrades deliberately instead of accidentally:
//!
//! - [`ColorMode::Ansi16`]: one 16-color-safe palette (named ANSI colors
//!   only, syntax highlighting off — its RGB output would be garbage).
//! - [`ColorMode::Monochrome`] (`NO_COLOR`): modifiers only — bold, dim,
//!   reversed — no color at all.
//!
//! Custom user themes (TOML, inheriting a base) are tracked in issue #15;
//! the theme struct is the stability surface they will build on.

use ratatui::style::{Color, Modifier, Style};

/// What the terminal can express. Detected by the binary from
/// `NO_COLOR`/`COLORTERM`/`TERM`; tests pick explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi16,
    Monochrome,
}

/// Names of the built-in themes, in documentation order.
pub const THEME_NAMES: [&str; 4] = ["ledger", "foolscap", "carbon", "blueprint"];

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
    /// syntect theme used for code coloring; `None` disables syntax
    /// highlighting (16-color and monochrome modes).
    pub syntax_theme: Option<&'static str>,
}

impl Default for Theme {
    fn default() -> Self {
        ledger()
    }
}

impl Theme {
    /// Resolve a theme name under a color mode. Unknown names yield `None`
    /// (callers report the valid list). Below truecolor, every name maps to
    /// the degraded palette — better identical than broken.
    pub fn resolve(name: &str, mode: ColorMode) -> Option<Theme> {
        let base = match name {
            "ledger" => ledger(),
            "foolscap" => foolscap(),
            "carbon" => carbon(),
            "blueprint" => blueprint(),
            _ => return None,
        };
        Some(match mode {
            ColorMode::TrueColor => base,
            ColorMode::Ansi16 => ansi16(),
            ColorMode::Monochrome => monochrome(),
        })
    }
}

fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// The default: a calm dark theme, green/red ink on subtle tints.
fn ledger() -> Theme {
    Theme {
        addition: Style::default().fg(Color::Green),
        deletion: Style::default().fg(Color::Red),
        context: Style::default(),
        addition_tint: Style::default().bg(rgb(0x0d3318)),
        deletion_tint: Style::default().bg(rgb(0x3d1517)),
        addition_emphasis: Style::default().bg(rgb(0x1c6b35)),
        deletion_emphasis: Style::default().bg(rgb(0x8b2d30)),
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
        syntax_theme: Some("base16-ocean.dark"),
    }
}

/// For light terminals: dark ink, paper-colored tints.
fn foolscap() -> Theme {
    Theme {
        addition: Style::default().fg(rgb(0x1a6a2e)),
        deletion: Style::default().fg(rgb(0x9c1f23)),
        context: Style::default(),
        addition_tint: Style::default().bg(rgb(0xdcf2dc)),
        deletion_tint: Style::default().bg(rgb(0xf8dcdc)),
        addition_emphasis: Style::default().bg(rgb(0xaee3ae)),
        deletion_emphasis: Style::default().bg(rgb(0xf2b3b3)),
        line_no: Style::default().fg(rgb(0x8a8a8a)),
        file_header: Style::default()
            .fg(Color::Black)
            .bg(rgb(0xe2e2e2))
            .add_modifier(Modifier::BOLD),
        hunk_header: Style::default().fg(rgb(0x1d4ed8)),
        meta: Style::default()
            .fg(rgb(0x8a8a8a))
            .add_modifier(Modifier::ITALIC),
        cursor_line: Style::default().bg(rgb(0xe6e6f2)),
        sidebar_title: Style::default()
            .fg(rgb(0x8a8a8a))
            .add_modifier(Modifier::BOLD),
        sidebar_selected: Style::default()
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
        status_bar: Style::default().fg(Color::White).bg(rgb(0x4b5563)),
        help_border: Style::default().fg(rgb(0x1d4ed8)),
        syntax_theme: Some("InspiredGitHub"),
    }
}

/// High-contrast dark: brighter ink, deeper tints.
fn carbon() -> Theme {
    Theme {
        addition: Style::default().fg(rgb(0x3ddc84)),
        deletion: Style::default().fg(rgb(0xff5f56)),
        context: Style::default().fg(rgb(0xd0d0d0)),
        addition_tint: Style::default().bg(rgb(0x06280f)),
        deletion_tint: Style::default().bg(rgb(0x330a0c)),
        addition_emphasis: Style::default().bg(rgb(0x14803c)),
        deletion_emphasis: Style::default().bg(rgb(0xa32226)),
        line_no: Style::default().fg(rgb(0x6b6b6b)),
        file_header: Style::default()
            .fg(Color::Black)
            .bg(rgb(0xd0d0d0))
            .add_modifier(Modifier::BOLD),
        hunk_header: Style::default()
            .fg(rgb(0xf0c674))
            .add_modifier(Modifier::BOLD),
        meta: Style::default()
            .fg(rgb(0x6b6b6b))
            .add_modifier(Modifier::ITALIC),
        cursor_line: Style::default().bg(rgb(0x303030)),
        sidebar_title: Style::default()
            .fg(rgb(0x6b6b6b))
            .add_modifier(Modifier::BOLD),
        sidebar_selected: Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        status_bar: Style::default().fg(Color::Black).bg(rgb(0xd0d0d0)),
        help_border: Style::default().fg(rgb(0xf0c674)),
        syntax_theme: Some("base16-eighties.dark"),
    }
}

/// Blue-tinted dark: the drafting-table look.
fn blueprint() -> Theme {
    Theme {
        addition: Style::default().fg(rgb(0x6fe3a1)),
        deletion: Style::default().fg(rgb(0xff8a8a)),
        context: Style::default().fg(rgb(0xb8cce0)),
        addition_tint: Style::default().bg(rgb(0x0c2e26)),
        deletion_tint: Style::default().bg(rgb(0x351a26)),
        addition_emphasis: Style::default().bg(rgb(0x176e4e)),
        deletion_emphasis: Style::default().bg(rgb(0x8e3050)),
        line_no: Style::default().fg(rgb(0x4a6a8a)),
        file_header: Style::default()
            .fg(rgb(0xdcecfb))
            .bg(rgb(0x102a43))
            .add_modifier(Modifier::BOLD),
        hunk_header: Style::default().fg(rgb(0x7fd1ff)),
        meta: Style::default()
            .fg(rgb(0x4a6a8a))
            .add_modifier(Modifier::ITALIC),
        cursor_line: Style::default().bg(rgb(0x16324d)),
        sidebar_title: Style::default()
            .fg(rgb(0x4a6a8a))
            .add_modifier(Modifier::BOLD),
        sidebar_selected: Style::default()
            .fg(rgb(0xdcecfb))
            .add_modifier(Modifier::BOLD),
        status_bar: Style::default().fg(rgb(0xdcecfb)).bg(rgb(0x102a43)),
        help_border: Style::default().fg(rgb(0x7fd1ff)),
        syntax_theme: Some("Solarized (dark)"),
    }
}

/// Named ANSI colors only; syntax highlighting off. Works everywhere.
fn ansi16() -> Theme {
    Theme {
        addition: Style::default().fg(Color::Green),
        deletion: Style::default().fg(Color::Red),
        context: Style::default(),
        addition_tint: Style::default(),
        deletion_tint: Style::default(),
        addition_emphasis: Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::REVERSED),
        deletion_emphasis: Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::REVERSED),
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
        syntax_theme: None,
    }
}

/// `NO_COLOR`: structure through weight alone.
fn monochrome() -> Theme {
    let plain = Style::default();
    Theme {
        addition: plain.add_modifier(Modifier::BOLD),
        deletion: plain.add_modifier(Modifier::DIM),
        context: plain,
        addition_tint: plain,
        deletion_tint: plain,
        addition_emphasis: plain.add_modifier(Modifier::REVERSED),
        deletion_emphasis: plain.add_modifier(Modifier::REVERSED),
        line_no: plain.add_modifier(Modifier::DIM),
        file_header: plain.add_modifier(Modifier::BOLD | Modifier::REVERSED),
        hunk_header: plain.add_modifier(Modifier::UNDERLINED),
        meta: plain.add_modifier(Modifier::DIM | Modifier::ITALIC),
        cursor_line: plain.add_modifier(Modifier::REVERSED),
        sidebar_title: plain.add_modifier(Modifier::BOLD),
        sidebar_selected: plain.add_modifier(Modifier::BOLD),
        status_bar: plain.add_modifier(Modifier::REVERSED),
        help_border: plain,
        syntax_theme: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_resolves_in_every_mode() {
        for name in THEME_NAMES {
            for mode in [
                ColorMode::TrueColor,
                ColorMode::Ansi16,
                ColorMode::Monochrome,
            ] {
                assert!(Theme::resolve(name, mode).is_some(), "{name} {mode:?}");
            }
        }
        assert!(Theme::resolve("nope", ColorMode::TrueColor).is_none());
    }

    #[test]
    fn degraded_modes_disable_syntax_and_rgb() {
        for name in THEME_NAMES {
            let theme = Theme::resolve(name, ColorMode::Ansi16).unwrap_or_default();
            assert_eq!(theme.syntax_theme, None);
            let mono = Theme::resolve(name, ColorMode::Monochrome).unwrap_or_default();
            assert_eq!(mono.syntax_theme, None);
            assert_eq!(mono.addition.fg, None, "monochrome means no color");
        }
    }
}
