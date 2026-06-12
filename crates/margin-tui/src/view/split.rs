//! Composition helpers for side-by-side rows: each visual row is a single
//! full-width line made of a fixed-width left half, a divider, and the
//! right half, so headers and the cursor bar span both panes naturally.

use unicode_width::UnicodeWidthChar;

/// Column budget for one half of a split row: line number (4) + space +
/// sign + content. Returns (number_width, content_width).
pub(crate) fn half_budget(half_width: usize) -> (usize, usize) {
    let number = 4;
    let content = half_width.saturating_sub(number + 2);
    (number, content)
}

/// Clip-or-pad already-styled spans to exactly `width` display columns,
/// preserving each segment's style. The styled twin of [`fit_to_width`].
pub(crate) fn fit_spans(
    spans: Vec<ratatui::text::Span<'static>>,
    width: usize,
) -> Vec<ratatui::text::Span<'static>> {
    let mut out: Vec<ratatui::text::Span<'static>> = Vec::with_capacity(spans.len() + 1);
    let mut cols = 0usize;
    'spans: for span in spans {
        if cols >= width {
            break;
        }
        let mut taken = String::new();
        for c in span.content.chars() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if cols + w > width {
                if !taken.is_empty() {
                    out.push(ratatui::text::Span::styled(taken, span.style));
                }
                break 'spans;
            }
            taken.push(c);
            cols += w;
        }
        if !taken.is_empty() {
            out.push(ratatui::text::Span::styled(taken, span.style));
        }
    }
    if cols < width {
        out.push(ratatui::text::Span::raw(" ".repeat(width - cols)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fit_spans;
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    fn flat(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn fits_pads_and_clips_by_display_width() {
        let raw = |s: &str| vec![Span::raw(s.to_string())];
        assert_eq!(flat(&fit_spans(raw("ab"), 4)), "ab  ");
        assert_eq!(flat(&fit_spans(raw("abcdef"), 4)), "abcd");
        // '你' is two columns: clipping must not split it.
        assert_eq!(flat(&fit_spans(raw("a\u{4f60}b"), 2)), "a ");
        assert_eq!(flat(&fit_spans(raw(""), 0)), "");
        assert_eq!(flat(&fit_spans(raw("xyz"), 0)), "");
        assert_eq!(flat(&fit_spans(Vec::new(), 3)), "   ");
    }

    #[test]
    fn styles_survive_clipping() {
        let spans = vec![
            Span::styled("ab".to_string(), Style::default().fg(Color::Blue)),
            Span::styled("cdef".to_string(), Style::default().fg(Color::Red)),
        ];
        let fitted = fit_spans(spans, 3);
        assert_eq!(flat(&fitted), "abc");
        assert_eq!(fitted[0].style.fg, Some(Color::Blue));
        assert_eq!(fitted[1].style.fg, Some(Color::Red));
        assert_eq!(fitted[1].content.as_ref(), "c");
    }
}
