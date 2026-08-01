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
//! Custom user themes (issue #15): a `[themes.<name>]` section in the
//! *user* config deserializes into [`CustomTheme`] — a built-in base
//! plus `#rrggbb` overrides. The schema lives here, beside [`Theme`],
//! so the color vocabulary and the struct evolve together; every field
//! name is a stability surface (docs/themes.md).

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
    /// The sidebar's staged-file indicator (a dot beside files with index
    /// content); reads as "staged", so it echoes the addition color.
    pub sidebar_staged: Style,
    pub status_bar: Style,
    pub help_border: Style,
    /// Search match highlight (`/` results).
    pub search_match: Style,
    /// syntect theme used for code coloring; `None` disables syntax
    /// highlighting (16-color and monochrome modes).
    pub syntax_theme: Option<String>,
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

/// A user-defined theme: a built-in base plus `#rrggbb` overrides
/// (issue #15). Deserialized from the user config's `[themes.<name>]`
/// by the binary; **every field name here is a stability surface**
/// (docs/themes.md documents the schema). Single-color keys override
/// the slot the base uses them in — ink keys set the foreground, tint
/// and highlight keys set the background — and keep the base's
/// modifiers (bold, italic) untouched. The two two-color surfaces get
/// explicit `_fg`/`_bg` keys.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomTheme {
    /// Built-in theme to inherit from; every unset key keeps its style.
    pub base: String,
    // Ink (foreground) keys.
    pub addition: Option<String>,
    pub deletion: Option<String>,
    pub context: Option<String>,
    pub line_no: Option<String>,
    pub hunk_header: Option<String>,
    pub meta: Option<String>,
    pub sidebar_title: Option<String>,
    pub sidebar_selected: Option<String>,
    pub sidebar_staged: Option<String>,
    pub help_border: Option<String>,
    // Background keys.
    pub addition_tint: Option<String>,
    pub deletion_tint: Option<String>,
    pub addition_emphasis: Option<String>,
    pub deletion_emphasis: Option<String>,
    pub cursor_line: Option<String>,
    pub search_match: Option<String>,
    // The two surfaces that paint both slots.
    pub file_header_fg: Option<String>,
    pub file_header_bg: Option<String>,
    pub status_bar_fg: Option<String>,
    pub status_bar_bg: Option<String>,
    /// syntect theme name (see docs/themes.md for the bundled list);
    /// unset inherits the base's.
    pub syntax_theme: Option<String>,
}

impl CustomTheme {
    /// Materialize under a color mode. `name` is the `[themes.<name>]`
    /// key, used only in error messages. TrueColor applies the
    /// overrides; degraded modes return the shared degraded palettes
    /// untouched — custom RGB in a 16-color terminal would render as
    /// garbage, exactly the accidental degradation ADR-0008 forbids.
    pub fn build(&self, name: &str, mode: ColorMode) -> Result<Theme, String> {
        match mode {
            ColorMode::Ansi16 => return Ok(ansi16()),
            ColorMode::Monochrome => return Ok(monochrome()),
            ColorMode::TrueColor => {}
        }
        let Some(mut theme) = Theme::resolve(&self.base, ColorMode::TrueColor) else {
            return Err(format!(
                "themes.{name}.base: '{}' is not a built-in theme ({})",
                self.base,
                THEME_NAMES.join(", ")
            ));
        };

        let fg = |style: Style, key: &str, value: &Option<String>| -> Result<Style, String> {
            Ok(match value {
                Some(hex) => style.fg(parse_hex(name, key, hex)?),
                None => style,
            })
        };
        let bg = |style: Style, key: &str, value: &Option<String>| -> Result<Style, String> {
            Ok(match value {
                Some(hex) => style.bg(parse_hex(name, key, hex)?),
                None => style,
            })
        };

        theme.addition = fg(theme.addition, "addition", &self.addition)?;
        theme.deletion = fg(theme.deletion, "deletion", &self.deletion)?;
        theme.context = fg(theme.context, "context", &self.context)?;
        theme.line_no = fg(theme.line_no, "line_no", &self.line_no)?;
        theme.hunk_header = fg(theme.hunk_header, "hunk_header", &self.hunk_header)?;
        theme.meta = fg(theme.meta, "meta", &self.meta)?;
        theme.sidebar_title = fg(theme.sidebar_title, "sidebar_title", &self.sidebar_title)?;
        theme.sidebar_selected = fg(
            theme.sidebar_selected,
            "sidebar_selected",
            &self.sidebar_selected,
        )?;
        theme.sidebar_staged = fg(theme.sidebar_staged, "sidebar_staged", &self.sidebar_staged)?;
        theme.help_border = fg(theme.help_border, "help_border", &self.help_border)?;

        theme.addition_tint = bg(theme.addition_tint, "addition_tint", &self.addition_tint)?;
        theme.deletion_tint = bg(theme.deletion_tint, "deletion_tint", &self.deletion_tint)?;
        theme.addition_emphasis = bg(
            theme.addition_emphasis,
            "addition_emphasis",
            &self.addition_emphasis,
        )?;
        theme.deletion_emphasis = bg(
            theme.deletion_emphasis,
            "deletion_emphasis",
            &self.deletion_emphasis,
        )?;
        theme.cursor_line = bg(theme.cursor_line, "cursor_line", &self.cursor_line)?;
        theme.search_match = bg(theme.search_match, "search_match", &self.search_match)?;

        theme.file_header = fg(theme.file_header, "file_header_fg", &self.file_header_fg)?;
        theme.file_header = bg(theme.file_header, "file_header_bg", &self.file_header_bg)?;
        theme.status_bar = fg(theme.status_bar, "status_bar_fg", &self.status_bar_fg)?;
        theme.status_bar = bg(theme.status_bar, "status_bar_bg", &self.status_bar_bg)?;

        if let Some(syntax) = &self.syntax_theme {
            let names = crate::highlight::syntax_theme_names();
            if !names.contains(&syntax.as_str()) {
                return Err(format!(
                    "themes.{name}.syntax_theme: '{syntax}' is not a bundled syntect theme ({})",
                    names.join(", ")
                ));
            }
            theme.syntax_theme = Some(syntax.clone());
        }
        Ok(theme)
    }
}

/// Strict `#rrggbb` (the only form docs/themes.md promises). The error
/// names the config key so a typo is a one-glance fix.
fn parse_hex(theme: &str, key: &str, value: &str) -> Result<Color, String> {
    let digits = value.strip_prefix('#').unwrap_or("");
    if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        if let Ok(hex) = u32::from_str_radix(digits, 16) {
            return Ok(rgb(hex));
        }
    }
    Err(format!(
        "themes.{theme}.{key}: '{value}' is not a '#rrggbb' color"
    ))
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
        sidebar_staged: Style::default().fg(Color::Green),
        status_bar: Style::default().fg(Color::Black).bg(Color::Gray),
        help_border: Style::default().fg(Color::Cyan),
        search_match: Style::default().bg(rgb(0x6b5d00)),
        syntax_theme: Some("base16-ocean.dark".into()),
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
        sidebar_staged: Style::default().fg(rgb(0x1a6a2e)),
        status_bar: Style::default().fg(Color::White).bg(rgb(0x4b5563)),
        help_border: Style::default().fg(rgb(0x1d4ed8)),
        search_match: Style::default().bg(rgb(0xffe9a0)),
        syntax_theme: Some("InspiredGitHub".into()),
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
        sidebar_staged: Style::default().fg(rgb(0x3ddc84)),
        status_bar: Style::default().fg(Color::Black).bg(rgb(0xd0d0d0)),
        help_border: Style::default().fg(rgb(0xf0c674)),
        search_match: Style::default().bg(rgb(0x806000)),
        syntax_theme: Some("base16-eighties.dark".into()),
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
        sidebar_staged: Style::default().fg(rgb(0x6fe3a1)),
        status_bar: Style::default().fg(rgb(0xdcecfb)).bg(rgb(0x102a43)),
        help_border: Style::default().fg(rgb(0x7fd1ff)),
        search_match: Style::default().bg(rgb(0x6e5d12)),
        syntax_theme: Some("Solarized (dark)".into()),
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
        sidebar_staged: Style::default().fg(Color::Green),
        status_bar: Style::default().fg(Color::Black).bg(Color::Gray),
        help_border: Style::default().fg(Color::Cyan),
        search_match: Style::default().fg(Color::Black).bg(Color::Yellow),
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
        sidebar_staged: plain.add_modifier(Modifier::BOLD),
        status_bar: plain.add_modifier(Modifier::REVERSED),
        help_border: plain,
        search_match: plain.add_modifier(Modifier::UNDERLINED),
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
    fn custom_theme_inherits_base_and_overrides_only_named_keys() {
        let custom = CustomTheme {
            base: "carbon".into(),
            addition: Some("#123456".into()),
            addition_tint: Some("#0d3318".into()),
            file_header_bg: Some("#222222".into()),
            syntax_theme: Some("base16-mocha.dark".into()),
            ..CustomTheme::default()
        };
        let theme = match custom.build("mocha", ColorMode::TrueColor) {
            Ok(theme) => theme,
            Err(err) => panic!("{err}"),
        };
        let base = Theme::resolve("carbon", ColorMode::TrueColor).unwrap_or_default();
        assert_eq!(theme.addition.fg, Some(rgb(0x123456)));
        assert_eq!(theme.addition_tint.bg, Some(rgb(0x0d3318)));
        assert_eq!(theme.deletion, base.deletion, "unset keys inherit");
        assert_eq!(
            theme.file_header.add_modifier, base.file_header.add_modifier,
            "overriding a color keeps the base's modifiers"
        );
        assert_eq!(theme.file_header.fg, base.file_header.fg);
        assert_eq!(theme.file_header.bg, Some(rgb(0x222222)));
        assert_eq!(theme.syntax_theme.as_deref(), Some("base16-mocha.dark"));
    }

    #[test]
    fn custom_theme_degrades_like_builtins() {
        // AC (issue #15): degraded modes apply regardless of custom
        // colors — RGB in a 16-color terminal is the accidental
        // degradation ADR-0008 forbids.
        let custom = CustomTheme {
            base: "ledger".into(),
            addition: Some("#123456".into()),
            ..CustomTheme::default()
        };
        let ansi = match custom.build("x", ColorMode::Ansi16) {
            Ok(theme) => theme,
            Err(err) => panic!("{err}"),
        };
        assert_eq!(ansi.addition.fg, Some(Color::Green), "no custom RGB");
        assert_eq!(ansi.syntax_theme, None);
        let mono = match custom.build("x", ColorMode::Monochrome) {
            Ok(theme) => theme,
            Err(err) => panic!("{err}"),
        };
        assert_eq!(mono.addition.fg, None);
    }

    #[test]
    fn custom_theme_errors_name_the_offending_key() {
        let bad_base = CustomTheme {
            base: "solarized".into(),
            ..CustomTheme::default()
        };
        let err = match bad_base.build("mine", ColorMode::TrueColor) {
            Err(err) => err,
            Ok(_) => panic!("unknown base must error"),
        };
        assert!(
            err.contains("themes.mine.base") && err.contains("ledger"),
            "{err}"
        );

        let bad_hex = CustomTheme {
            base: "ledger".into(),
            deletion_tint: Some("red".into()),
            ..CustomTheme::default()
        };
        let err = match bad_hex.build("mine", ColorMode::TrueColor) {
            Err(err) => err,
            Ok(_) => panic!("bad hex must error"),
        };
        assert!(
            err.contains("themes.mine.deletion_tint") && err.contains("#rrggbb"),
            "{err}"
        );

        let bad_syntax = CustomTheme {
            base: "ledger".into(),
            syntax_theme: Some("nope".into()),
            ..CustomTheme::default()
        };
        let err = match bad_syntax.build("mine", ColorMode::TrueColor) {
            Err(err) => err,
            Ok(_) => panic!("unknown syntect theme must error"),
        };
        assert!(
            err.contains("themes.mine.syntax_theme") && err.contains("base16-ocean.dark"),
            "available names must be listed: {err}"
        );
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
