//! Span composition: layer syntax colors, intra-line emphasis, and search
//! highlights over a base style without ever splitting a multi-byte
//! character.

use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::Span;

/// Re-style byte `ranges` of already-composed spans by patching `patch`
/// over the affected segments. Offsets are into the concatenation of the
/// span texts. All boundaries (span pieces, ranges) originate from `&str`
/// slicing and are therefore char-aligned; this preserves that.
pub(crate) fn overlay(
    spans: Vec<Span<'static>>,
    ranges: &[Range<usize>],
    patch: Style,
) -> Vec<Span<'static>> {
    if ranges.is_empty() {
        return spans;
    }
    let mut out = Vec::with_capacity(spans.len() + ranges.len() * 2);
    let mut offset = 0usize;
    for span in spans {
        let text = span.content.into_owned();
        let len = text.len();
        let mut start = 0usize;
        while start < len {
            let abs = offset + start;
            let inside = ranges.iter().any(|r| r.contains(&abs));
            // Segment ends at the piece end or the nearest range edge.
            let mut end_abs = offset + len;
            for range in ranges {
                if range.start > abs {
                    end_abs = end_abs.min(range.start);
                } else if range.contains(&abs) {
                    end_abs = end_abs.min(range.end);
                }
            }
            let end = end_abs - offset;
            let segment = text.get(start..end).unwrap_or_default().to_string();
            let style = if inside {
                span.style.patch(patch)
            } else {
                span.style
            };
            if !segment.is_empty() {
                out.push(Span::styled(segment, style));
            }
            start = end;
        }
        offset += len;
    }
    out
}

/// Build the styled spans for one line's content: base style (fg in plain
/// mode, bg tint under syntax), syntax pieces patched over it, then
/// intra-line emphasis patched over the changed words.
pub(crate) fn compose_content(
    content: &str,
    syntax: Option<Vec<(Style, String)>>,
    emphasis: &[Range<usize>],
    base: Style,
    emphasis_patch: Style,
) -> Vec<Span<'static>> {
    let pieces: Vec<Span<'static>> = match syntax {
        Some(spans) => spans
            .into_iter()
            .map(|(style, text)| Span::styled(text, base.patch(style)))
            .collect(),
        None if content.is_empty() => Vec::new(),
        None => vec![Span::styled(content.to_string(), base)],
    };
    overlay(pieces, emphasis, emphasis_patch)
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)] // range literals here ARE byte ranges
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn flat(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn plain_with_emphasis_splits_at_range_edges() {
        let base = Style::default().fg(Color::Green);
        let emph = Style::default().bg(Color::Red);
        let spans = compose_content("let a = 1;", None, &[8..9], base, emph);
        assert_eq!(flat(&spans), "let a = 1;");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content.as_ref(), "1");
        assert_eq!(spans[1].style.bg, Some(Color::Red));
        assert_eq!(spans[0].style.bg, None);
    }

    #[test]
    fn syntax_pieces_keep_their_fg_and_gain_base_bg() {
        let base = Style::default().bg(Color::Black);
        let syntax = vec![
            (Style::default().fg(Color::Blue), "let ".to_string()),
            (Style::default().fg(Color::White), "a = 1;".to_string()),
        ];
        let spans = compose_content("let a = 1;", Some(syntax), &[], base, Style::default());
        assert_eq!(flat(&spans), "let a = 1;");
        assert_eq!(spans[0].style.fg, Some(Color::Blue));
        assert_eq!(spans[0].style.bg, Some(Color::Black));
    }

    #[test]
    fn emphasis_crossing_a_syntax_boundary_splits_both() {
        let syntax = vec![
            (Style::default().fg(Color::Blue), "abc".to_string()),
            (Style::default().fg(Color::White), "def".to_string()),
        ];
        let emph = Style::default().bg(Color::Red);
        let spans = compose_content("abcdef", Some(syntax), &[2..4], Style::default(), emph);
        assert_eq!(flat(&spans), "abcdef");
        let emphasized: String = spans
            .iter()
            .filter(|s| s.style.bg == Some(Color::Red))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(emphasized, "cd");
    }

    #[test]
    fn overlays_stack_search_on_top_of_emphasis() {
        let base = Style::default().fg(Color::Green);
        let emph = Style::default().bg(Color::Red);
        let search = Style::default().bg(Color::Yellow);
        let spans = compose_content("abcdef", None, &[1..4], base, emph);
        let spans = overlay(spans, &[3..5], search);
        assert_eq!(flat(&spans), "abcdef");
        // byte 3 was emphasized AND matches search: search bg wins (last patch).
        let at = |needle: &str| {
            spans
                .iter()
                .find(|s| s.content.as_ref() == needle)
                .unwrap_or_else(|| panic!("segment {needle:?} missing in {spans:?}"))
                .style
        };
        assert_eq!(at("bc").bg, Some(Color::Red));
        assert_eq!(at("d").bg, Some(Color::Yellow));
        assert_eq!(at("e").bg, Some(Color::Yellow));
        assert_eq!(at("f").bg, None);
    }

    #[test]
    fn empty_content_yields_no_spans() {
        assert!(compose_content("", None, &[], Style::default(), Style::default()).is_empty());
    }
}
