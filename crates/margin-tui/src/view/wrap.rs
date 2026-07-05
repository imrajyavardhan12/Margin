//! Line wrapping for the `w` toggle (issue #14).
//!
//! One boundary rule, one owner: [`RowFill`] decides where visual rows
//! break, and both [`wrap_spans`] (renderer) and [`wrap_count`] (scroll
//! math in `app.rs`) are thin walks over it — they cannot drift apart.
//! The rule is a greedy fill by display width where a wide character that
//! doesn't fit starts the next row (so a row can end one column short).
//! Ceil-division over the total width gets CJK content wrong; never
//! "simplify" to it.
//!
//! Both functions take a `cap`: rows beyond it are never materialized, so
//! per-frame work is bounded by the viewport, not by the longest line on
//! screen. Truncation at the cap is safe because every caller either
//! discards rows past the viewport (renderer) or compares heights against
//! a budget smaller than the cap (scroll math).

use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;

/// The greedy row-fill rule. Feed each character's display width in order;
/// `breaks` answers whether that character starts a new visual row.
/// `width == 0` disables breaking (the caller's budget collapsed; ratatui
/// clips the overlong row).
pub(crate) struct RowFill {
    width: usize,
    cols: usize,
}

impl RowFill {
    pub(crate) fn new(width: usize) -> Self {
        Self { width, cols: 0 }
    }

    pub(crate) fn breaks(&mut self, char_width: usize) -> bool {
        if self.width == 0 {
            return false;
        }
        if self.cols + char_width > self.width {
            self.cols = char_width;
            true
        } else {
            self.cols += char_width;
            false
        }
    }
}

/// Split styled spans into visual rows of at most `width` display columns,
/// producing at most `cap` rows (content past the cap is dropped — it is
/// off-screen by construction). Always returns at least one row.
pub(crate) fn wrap_spans(
    spans: Vec<Span<'static>>,
    width: usize,
    cap: usize,
) -> Vec<Vec<Span<'static>>> {
    let cap = cap.max(1);
    let mut fill = RowFill::new(width);
    let mut rows: Vec<Vec<Span<'static>>> = Vec::new();
    let mut row: Vec<Span<'static>> = Vec::new();
    'spans: for span in spans {
        let mut taken = String::new();
        for c in span.content.chars() {
            if fill.breaks(UnicodeWidthChar::width(c).unwrap_or(0)) {
                if !taken.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut taken), span.style));
                }
                rows.push(std::mem::take(&mut row));
                if rows.len() == cap {
                    break 'spans;
                }
            }
            taken.push(c);
        }
        if !taken.is_empty() {
            row.push(Span::styled(taken, span.style));
        }
    }
    if rows.len() < cap {
        rows.push(row);
    }
    rows
}

/// How many visual rows `wrap_spans` would produce for this text at this
/// width, saturating at `cap` — the same [`RowFill`] walk, no allocation.
pub(crate) fn wrap_count(text: &str, width: usize, cap: usize) -> usize {
    let cap = cap.max(1);
    let mut fill = RowFill::new(width);
    let mut rows = 1usize;
    for c in text.chars() {
        if fill.breaks(UnicodeWidthChar::width(c).unwrap_or(0)) {
            rows += 1;
            if rows >= cap {
                return cap;
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{wrap_count, wrap_spans};
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    const NO_CAP: usize = usize::MAX;

    fn flat(rows: &[Vec<Span<'_>>]) -> Vec<String> {
        rows.iter()
            .map(|row| row.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wraps_by_display_width() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        assert_eq!(
            flat(&wrap_spans(raw("abcdef"), 4, NO_CAP)),
            vec!["abcd", "ef"]
        );
        assert_eq!(flat(&wrap_spans(raw("abcd"), 4, NO_CAP)), vec!["abcd"]);
        assert_eq!(flat(&wrap_spans(raw(""), 4, NO_CAP)), vec![""]);
        // Width 0: give up chunking rather than loop.
        assert_eq!(flat(&wrap_spans(raw("abc"), 0, NO_CAP)), vec!["abc"]);
    }

    #[test]
    fn wide_chars_break_early_not_mid_glyph() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        // '你' is 2 columns: three of them at width 3 need 3 rows, not
        // ceil(6/3) = 2 — each row ends one column short.
        assert_eq!(
            flat(&wrap_spans(raw("\u{4f60}\u{4f60}\u{4f60}"), 3, NO_CAP)),
            vec!["\u{4f60}", "\u{4f60}", "\u{4f60}"]
        );
    }

    #[test]
    fn styles_survive_wrapping() {
        let spans = vec![
            Span::styled("abc".to_string(), Style::default().fg(Color::Blue)),
            Span::styled("def".to_string(), Style::default().fg(Color::Red)),
        ];
        let rows = wrap_spans(spans, 4, NO_CAP);
        assert_eq!(flat(&rows), vec!["abcd", "ef"]);
        assert_eq!(rows[0][0].style.fg, Some(Color::Blue));
        assert_eq!(rows[0][1].style.fg, Some(Color::Red));
        assert_eq!(rows[1][0].style.fg, Some(Color::Red));
    }

    #[test]
    fn cap_bounds_rows_and_count_agrees() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        // 12 chars at width 3 = 4 true rows; cap at 2 keeps exactly 2.
        let text = "abcdefghijkl";
        assert_eq!(flat(&wrap_spans(raw(text), 3, 2)), vec!["abc", "def"]);
        assert_eq!(wrap_count(text, 3, 2), 2);
        // Cap above the true count changes nothing.
        assert_eq!(wrap_spans(raw(text), 3, 9).len(), 4);
        assert_eq!(wrap_count(text, 3, 9), 4);
        // Degenerate cap still yields one row.
        assert_eq!(wrap_spans(raw(text), 3, 0).len(), 1);
        assert_eq!(wrap_count(text, 3, 0), 1);
    }

    #[test]
    fn count_matches_spans_for_any_mix() {
        let cases = [
            ("", 4),
            ("abc", 4),
            ("abcdef", 4),
            ("\u{4f60}\u{4f60}\u{4f60}", 3),
            ("a\u{4f60}b\u{4f60}c", 2),
            ("mixed \u{4f60}\u{597d} text with width", 5),
            ("x", 1),
            ("tab\tand\u{0}nul", 3), // zero-width controls
            ("ends exactly at bound", 7),
        ];
        for (text, width) in cases {
            for cap in [1, 2, 3, NO_CAP] {
                let spans = vec![Span::raw(text.to_string())];
                assert_eq!(
                    wrap_spans(spans, width, cap).len(),
                    wrap_count(text, width, cap),
                    "count diverged for {text:?} at width {width} cap {cap}"
                );
            }
        }
    }
}
