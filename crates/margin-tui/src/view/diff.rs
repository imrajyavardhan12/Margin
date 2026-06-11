//! The main pane: the unified review stream (issue #3 adds side-by-side).

use margin_core::{FileDiff, FileStatus, LineKind};
use ratatui::layout::Rect;
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

    let height = usize::from(area.height);
    let end = usize::min(state.rows.len(), state.scroll + height);
    let mut lines: Vec<TLine> = Vec::with_capacity(height);

    for idx in state.scroll..end {
        let Some(row) = state.rows.get(idx) else {
            break;
        };
        // The cursor marker is a glyph, not just a style, so it survives
        // 16-color terminals and shows up in text snapshots.
        let marker = if idx == state.cursor { "\u{258c}" } else { " " };
        let mut line = match *row {
            Row::FileHeader { file } => file_header(state, file, marker),
            Row::Meta { file } => meta_row(state, file, marker),
            Row::HunkHeader { file, hunk } => hunk_header(state, file, hunk, marker),
            Row::Line {
                file,
                hunk,
                line,
                old_no,
                new_no,
            } => diff_line(state, file, hunk, line, old_no, new_no, marker),
        };
        if idx == state.cursor {
            line = line.style(state.theme.cursor_line);
        }
        lines.push(line);
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
    let mut text = format!(
        "{marker}{} {}  +{} -{}",
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

#[allow(clippy::too_many_arguments)] // private helper mirroring Row::Line's payload
fn diff_line(
    state: &AppState,
    file: usize,
    hunk: usize,
    line: usize,
    old_no: Option<u32>,
    new_no: Option<u32>,
    marker: &str,
) -> TLine<'static> {
    let Some(l) = state
        .changeset
        .files
        .get(file)
        .and_then(|f| f.hunks.get(hunk))
        .and_then(|h| h.lines.get(line))
    else {
        return TLine::from(marker.to_string());
    };

    let (sign, style) = match l.kind {
        LineKind::Addition => ("+", state.theme.addition),
        LineKind::Deletion => ("-", state.theme.deletion),
        LineKind::Context => (" ", state.theme.context),
    };
    let numbers = format!(
        "{:>4} {:>4} ",
        old_no.map(|n| n.to_string()).unwrap_or_default(),
        new_no.map(|n| n.to_string()).unwrap_or_default(),
    );
    let mut content = printable(&l.content);
    if l.no_newline {
        content.push_str(" \u{2205}"); // mark missing trailing newline
    }

    TLine::from(vec![
        Span::raw(marker.to_string()),
        Span::styled(numbers, state.theme.line_no),
        Span::styled(format!("{sign}{content}"), style),
    ])
}
