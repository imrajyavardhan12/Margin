//! Line wrapping for the `w` toggle (issue #14).
//!
//! Two functions, one algorithm: `wrap_spans` chunks styled spans into
//! visual rows for the renderer; `wrap_count` predicts how many rows that
//! produces for the scroll math in `app.rs`. They MUST agree — a greedy
//! fill by display width, where a wide character that doesn't fit starts
//! the next row (so a row can end one column short). Ceil-division over
//! the total width gets CJK content wrong; never "simplify" to it.

use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;

/// Split styled spans into visual rows of at most `width` display columns.
/// Always returns at least one row (an empty line still occupies a row).
/// `width == 0` disables chunking (the caller's budget collapsed; ratatui
/// clips the overlong row).
pub(crate) fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    if width == 0 {
        return vec![spans];
    }
    let mut rows: Vec<Vec<Span<'static>>> = Vec::new();
    let mut row: Vec<Span<'static>> = Vec::new();
    let mut cols = 0usize;
    for span in spans {
        let mut taken = String::new();
        for c in span.content.chars() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if cols + w > width {
                if !taken.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut taken), span.style));
                }
                rows.push(std::mem::take(&mut row));
                cols = 0;
            }
            taken.push(c);
            cols += w;
        }
        if !taken.is_empty() {
            row.push(Span::styled(taken, span.style));
        }
    }
    rows.push(row);
    rows
}

/// How many visual rows `wrap_spans` would produce for this text at this
/// width — same greedy scan, no styling or allocation.
pub(crate) fn wrap_count(text: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut cols = 0usize;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if cols + w > width {
            rows += 1;
            cols = 0;
        }
        cols += w;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{wrap_count, wrap_spans};
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    fn flat(rows: &[Vec<Span<'_>>]) -> Vec<String> {
        rows.iter()
            .map(|row| row.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wraps_by_display_width() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        assert_eq!(flat(&wrap_spans(raw("abcdef"), 4)), vec!["abcd", "ef"]);
        assert_eq!(flat(&wrap_spans(raw("abcd"), 4)), vec!["abcd"]);
        assert_eq!(flat(&wrap_spans(raw(""), 4)), vec![""]);
        // Width 0: give up chunking rather than loop.
        assert_eq!(flat(&wrap_spans(raw("abc"), 0)), vec!["abc"]);
    }

    #[test]
    fn wide_chars_break_early_not_mid_glyph() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        // '你' is 2 columns: three of them at width 3 need 3 rows, not
        // ceil(6/3) = 2 — each row ends one column short.
        assert_eq!(
            flat(&wrap_spans(raw("\u{4f60}\u{4f60}\u{4f60}"), 3)),
            vec!["\u{4f60}", "\u{4f60}", "\u{4f60}"]
        );
    }

    #[test]
    fn styles_survive_wrapping() {
        let spans = vec![
            Span::styled("abc".to_string(), Style::default().fg(Color::Blue)),
            Span::styled("def".to_string(), Style::default().fg(Color::Red)),
        ];
        let rows = wrap_spans(spans, 4);
        assert_eq!(flat(&rows), vec!["abcd", "ef"]);
        assert_eq!(rows[0][0].style.fg, Some(Color::Blue));
        assert_eq!(rows[0][1].style.fg, Some(Color::Red));
        assert_eq!(rows[1][0].style.fg, Some(Color::Red));
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
        ];
        for (text, width) in cases {
            let spans = vec![Span::raw(text.to_string())];
            assert_eq!(
                wrap_spans(spans, width).len(),
                wrap_count(text, width),
                "count diverged for {text:?} at width {width}"
            );
        }
    }
}
