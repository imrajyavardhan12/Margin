//! Span composition: layer syntax colors and intra-line emphasis over a
//! base style without ever splitting a multi-byte character.

use std::ops::Range;

use ratatui::style::Style;
use ratatui::text::Span;

/// Build the styled spans for one line's content.
///
/// - `syntax`: foreground-colored pieces covering `content` exactly (from
///   the highlight cache), or `None` to render plain.
/// - `emphasis`: byte ranges of `content` to re-style (changed words).
/// - `base`: applied under everything (fg for plain mode, bg tint for
///   syntax mode); syntax styles are patched over it.
/// - `emphasis_patch`: patched over emphasized segments.
///
/// All boundaries (syntax pieces, emphasis ranges) originate from `&str`
/// slicing and are therefore char-aligned; this function preserves that.
pub(crate) fn compose_content(
    content: &str,
    syntax: Option<Vec<(Style, String)>>,
    emphasis: &[Range<usize>],
    base: Style,
    emphasis_patch: Style,
) -> Vec<Span<'static>> {
    let pieces: Vec<(Style, String)> = match syntax {
        Some(spans) => spans
            .into_iter()
            .map(|(style, text)| (base.patch(style), text))
            .collect(),
        None => vec![(base, content.to_string())],
    };

    let mut out = Vec::with_capacity(pieces.len() + emphasis.len() * 2);
    let mut offset = 0usize;
    for (style, text) in pieces {
        let len = text.len();
        let mut start = 0usize;
        while start < len {
            let abs = offset + start;
            let emphasized = emphasis.iter().any(|r| r.contains(&abs));
            // Segment ends at the piece end or the nearest emphasis edge.
            let mut end_abs = offset + len;
            for range in emphasis {
                if range.start > abs {
                    end_abs = end_abs.min(range.start);
                } else if range.contains(&abs) {
                    end_abs = end_abs.min(range.end);
                }
            }
            let end = end_abs - offset;
            let segment = text.get(start..end).unwrap_or_default().to_string();
            let segment_style = if emphasized {
                style.patch(emphasis_patch)
            } else {
                style
            };
            if !segment.is_empty() {
                out.push(Span::styled(segment, segment_style));
            }
            start = end;
        }
        offset += len;
    }
    out
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
    fn empty_content_yields_no_spans() {
        assert!(compose_content("", None, &[], Style::default(), Style::default()).is_empty());
    }
}
