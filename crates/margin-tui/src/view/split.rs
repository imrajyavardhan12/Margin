//! Composition helpers for side-by-side rows: each visual row is a single
//! full-width line made of a fixed-width left half, a divider, and the
//! right half, so headers and the cursor bar span both panes naturally.

use unicode_width::UnicodeWidthChar;

/// Clip-or-pad `text` to exactly `width` display columns (unicode-aware:
/// CJK and emoji occupy two cells; padding a double-width char that does
/// not fit keeps columns aligned).
pub(crate) fn fit_to_width(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(width);
    let mut cols = 0;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if cols + w > width {
            break;
        }
        out.push(c);
        cols += w;
    }
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(cols)));
    out
}

/// Column budget for one half of a split row: line number (4) + space +
/// sign + content. Returns (number_width, content_width).
pub(crate) fn half_budget(half_width: usize) -> (usize, usize) {
    let number = 4;
    let content = half_width.saturating_sub(number + 2);
    (number, content)
}

#[cfg(test)]
mod tests {
    use super::fit_to_width;

    #[test]
    fn fits_pads_and_clips_by_display_width() {
        assert_eq!(fit_to_width("ab", 4), "ab  ");
        assert_eq!(fit_to_width("abcdef", 4), "abcd");
        // '你' is two columns: clipping must not split it.
        assert_eq!(fit_to_width("a\u{4f60}b", 2), "a ");
        assert_eq!(fit_to_width("", 3), "   ");
        assert_eq!(fit_to_width("xyz", 0), "");
    }
}
