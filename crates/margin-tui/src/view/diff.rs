//! The main pane: the unified review stream (issue #3 adds side-by-side).

use margin_core::{FileDiff, FileStatus, LineKind};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line as TLine, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::printable;
use crate::app::{AppState, Row};

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    if state.rows.is_empty() {
        let empty = Paragraph::new("\n\n   no changes").style(state.theme.meta);
        frame.render_widget(empty, area);
        return;
    }

    // Rows are 1 visual line tall unless wrap is on; walk until the
    // viewport is full, truncating the last row's tail if it overflows.
    let height = usize::from(area.height);
    let mut lines: Vec<TLine> = Vec::with_capacity(height);
    let mut idx = state.scroll;
    while lines.len() < height && idx < state.rows.len() {
        let Some(row) = state.rows.get(idx) else {
            break;
        };
        // The cursor marker is a glyph, not just a style, so it survives
        // 16-color terminals and shows up in text snapshots.
        let marker = if idx == state.cursor { "\u{258c}" } else { " " };
        // Wrapped rows only ever materialize what still fits on screen.
        let cap = height - lines.len();
        let row_lines = match *row {
            Row::FileHeader { file } => vec![file_header(state, file, marker)],
            Row::Meta { file } => vec![meta_row(state, file, marker)],
            Row::HunkHeader { file, hunk } => vec![hunk_header(state, file, hunk, marker)],
            Row::Line {
                file,
                hunk,
                line,
                old_no,
                new_no,
            } => diff_line(
                state, file, hunk, line, old_no, new_no, marker, area.width, cap,
            ),
            Row::Split {
                file,
                hunk,
                left,
                right,
            } => split_line(state, file, hunk, left, right, marker, area.width, cap),
        };
        for mut line in row_lines {
            if lines.len() == height {
                break;
            }
            if idx == state.cursor {
                line = line.style(state.theme.cursor_line);
            }
            lines.push(line);
        }
        idx += 1;
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn status_glyph(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "A",
        FileStatus::Deleted => "D",
        FileStatus::Modified => "M",
        FileStatus::Renamed => "R",
        FileStatus::Copied => "C",
    }
}

fn file_label(diff: &FileDiff) -> String {
    match (&diff.old_path, &diff.new_path, diff.status) {
        (Some(old), Some(new), FileStatus::Renamed | FileStatus::Copied) if old != new => {
            format!("{} \u{2192} {}", printable(old), printable(new))
        }
        _ => diff.display_path().into_owned(),
    }
}

fn file_header(state: &AppState, file: usize, marker: &str) -> TLine<'static> {
    let Some(diff) = state.changeset.files.get(file) else {
        return TLine::from(marker.to_string());
    };
    // Collapsed files carry a fold marker; counts stay visible (#21).
    let fold = if state.is_collapsed(file) {
        "\u{25b8} "
    } else {
        ""
    };
    let mut text = format!(
        "{marker}{fold}{} {}  +{} -{}",
        status_glyph(diff.status),
        file_label(diff),
        diff.additions(),
        diff.deletions()
    );
    if diff.is_binary {
        text.push_str("  (binary)");
    }
    if let (Some(old), Some(new)) = (diff.old_mode, diff.new_mode) {
        if old != new {
            text.push_str(&format!("  {old:o} \u{2192} {new:o}"));
        }
    }
    TLine::styled(text, state.theme.file_header)
}

fn meta_row(state: &AppState, file: usize, marker: &str) -> TLine<'static> {
    let Some(diff) = state.changeset.files.get(file) else {
        return TLine::from(marker.to_string());
    };
    let text = if diff.is_binary {
        "binary file: contents not shown"
    } else if matches!(diff.status, FileStatus::Renamed | FileStatus::Copied) {
        "renamed without content changes"
    } else if diff.old_mode != diff.new_mode {
        "mode change only"
    } else {
        "no content changes"
    };
    TLine::styled(format!("{marker}      \u{2514} {text}"), state.theme.meta)
}

fn hunk_header(state: &AppState, file: usize, hunk: usize, marker: &str) -> TLine<'static> {
    let Some(h) = state
        .changeset
        .files
        .get(file)
        .and_then(|f| f.hunks.get(hunk))
    else {
        return TLine::from(marker.to_string());
    };
    let mut text = format!(
        "{marker}@@ -{},{} +{},{} @@",
        h.old_start, h.old_count, h.new_start, h.new_count
    );
    if let Some(heading) = &h.heading {
        text.push(' ');
        text.push_str(&printable(heading));
    }
    TLine::styled(text, state.theme.hunk_header)
}

/// Sign char, base style, and emphasis patch for a line kind. The base is
/// the flat fg color in plain mode and a bg tint under syntax colors.
fn line_styles(state: &AppState, kind: LineKind, has_syntax: bool) -> (&'static str, Style, Style) {
    match kind {
        LineKind::Addition => (
            "+",
            if has_syntax {
                state.theme.addition_tint
            } else {
                state.theme.addition
            },
            state.theme.addition_emphasis,
        ),
        LineKind::Deletion => (
            "-",
            if has_syntax {
                state.theme.deletion_tint
            } else {
                state.theme.deletion
            },
            state.theme.deletion_emphasis,
        ),
        LineKind::Context => (" ", state.theme.context, Style::default()),
    }
}

/// Resolve a hunk line to everything rendering needs. `None` when any
/// index is out of bounds (rendered as an empty/blank row upstream).
fn resolve_line(
    state: &AppState,
    file: usize,
    hunk: usize,
    line: usize,
) -> Option<(&FileDiff, &margin_core::Hunk, &margin_core::Line)> {
    state
        .changeset
        .files
        .get(file)
        .and_then(|f| f.hunks.get(hunk).map(|h| (f, h)))
        .and_then(|(f, h)| h.lines.get(line).map(|l| (f, h, l)))
}

/// THE content pipeline: syntax + intra-line emphasis + search overlay +
/// the no-newline suffix, as styled spans, plus the sign/base style for
/// the gutter. Every rendered line — unified or split, wrapped or not —
/// comes from here, and `AppState::line_wrap_count` measures the same
/// text (`printable` + `NO_NEWLINE_SUFFIX`); see the AGENTS.md gotcha.
fn composed_line_spans(
    state: &AppState,
    file: usize,
    hunk: usize,
    line: usize,
) -> Option<(Vec<Span<'static>>, &'static str, Style)> {
    let (file_diff, h, l) = resolve_line(state, file, hunk, line)?;
    let render = state
        .highlight
        .line_render(file, hunk, &file_diff.display_path(), h, line);
    let (sign, base, emphasis_patch) = line_styles(state, l.kind, render.syntax.is_some());
    let content = printable(&l.content);
    let mut spans = super::style::compose_content(
        &content,
        render.syntax,
        &render.emphasis,
        base,
        emphasis_patch,
    );
    spans = highlight_matches(state, &content, spans);
    if l.no_newline {
        spans.push(Span::styled(
            super::NO_NEWLINE_SUFFIX.to_string(),
            state.theme.meta,
        ));
    }
    Some((spans, sign, base))
}

#[allow(clippy::too_many_arguments)] // private helper mirroring Row::Line's payload
fn diff_line(
    state: &AppState,
    file: usize,
    hunk: usize,
    line: usize,
    old_no: Option<u32>,
    new_no: Option<u32>,
    marker: &str,
    width: u16,
    cap: usize,
) -> Vec<TLine<'static>> {
    let Some((content_spans, sign, base)) = composed_line_spans(state, file, hunk, line) else {
        return vec![TLine::from(marker.to_string())];
    };
    let numbers = format!(
        "{:>4} {:>4} ",
        old_no.map(|n| n.to_string()).unwrap_or_default(),
        new_no.map(|n| n.to_string()).unwrap_or_default(),
    );
    let budget = usize::from(width).saturating_sub(super::UNIFIED_PREFIX_COLS);
    let chunks = if state.wrap {
        super::wrap::wrap_spans(content_spans, budget, cap)
    } else {
        vec![content_spans] // single row; ratatui clips at the pane edge
    };
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut spans = if i == 0 {
                vec![
                    Span::raw(marker.to_string()),
                    Span::styled(numbers.clone(), state.theme.line_no),
                    Span::styled(sign.to_string(), base),
                ]
            } else {
                vec![Span::raw(" ".repeat(super::UNIFIED_PREFIX_COLS))]
            };
            spans.extend(chunk);
            TLine::from(spans)
        })
        .collect()
}

/// Patch the search-match style over any regex hits in this content.
fn highlight_matches(
    state: &AppState,
    content: &str,
    spans: Vec<Span<'static>>,
) -> Vec<Span<'static>> {
    let Some(regex) = state.search_regex() else {
        return spans;
    };
    let ranges: Vec<_> = regex
        .find_iter(content)
        .map(|m| m.range())
        .filter(|r| !r.is_empty())
        .collect();
    super::style::overlay(spans, &ranges, state.theme.search_match)
}

/// One side-by-side visual row: `marker │ old half │ divider │ new half`,
/// composed as a single full-width line so the cursor bar spans both panes.
/// With wrap on, both halves chunk independently and the row is as tall as
/// its taller half; the shorter side pads with blanks.
#[allow(clippy::too_many_arguments)] // private helper mirroring Row::Split's payload
fn split_line(
    state: &AppState,
    file: usize,
    hunk: usize,
    left: Option<(usize, u32)>,
    right: Option<(usize, u32)>,
    marker: &str,
    width: u16,
    cap: usize,
) -> Vec<TLine<'static>> {
    let (left_width, right_width) = super::split_halves(usize::from(width));
    let left_rows = half_rows(state, file, hunk, left, left_width, cap);
    let right_rows = half_rows(state, file, hunk, right, right_width, cap);
    let rows = left_rows.len().max(right_rows.len());
    let mut left_rows = left_rows.into_iter();
    let mut right_rows = right_rows.into_iter();
    (0..rows)
        .map(|i| {
            let mut spans = vec![Span::raw(if i == 0 {
                marker.to_string()
            } else {
                " ".to_string()
            })];
            spans.extend(left_rows.next().unwrap_or_else(|| blank_half(left_width)));
            spans.push(Span::styled("\u{2502}".to_string(), state.theme.line_no));
            spans.extend(right_rows.next().unwrap_or_else(|| blank_half(right_width)));
            TLine::from(spans)
        })
        .collect()
}

fn blank_half(half_width: usize) -> Vec<Span<'static>> {
    vec![Span::raw(" ".repeat(half_width))]
}

/// One half of a split row, pre-assembled as 1..=cap full-width sub-rows:
/// gutter (line number + sign) on the first, blanks after, each fitted to
/// exactly `half_width` columns. An absent or unresolvable side is one
/// blank sub-row (the zip in `split_line` pads the rest).
fn half_rows(
    state: &AppState,
    file: usize,
    hunk: usize,
    side: Option<(usize, u32)>,
    half_width: usize,
    cap: usize,
) -> Vec<Vec<Span<'static>>> {
    let (number_width, content_width) = super::split::half_budget(half_width);
    let gutter_cols = number_width + 2; // number + space + sign
    let Some((line, no)) = side else {
        return vec![blank_half(half_width)];
    };
    let Some((content_spans, sign, base)) = composed_line_spans(state, file, hunk, line) else {
        return vec![blank_half(half_width)];
    };
    let chunks = if state.wrap {
        super::wrap::wrap_spans(content_spans, content_width, cap)
    } else {
        vec![content_spans]
    };
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut row = if i == 0 {
                vec![
                    Span::styled(
                        format!("{:>width$} ", no, width = number_width),
                        state.theme.line_no,
                    ),
                    Span::styled(sign.to_string(), base),
                ]
            } else {
                vec![Span::raw(" ".repeat(gutter_cols.min(half_width)))]
            };
            row.extend(super::split::fit_spans(chunk, content_width));
            row
        })
        .collect()
}
