//! The file list: one row per file, current file highlighted.

use margin_core::FileStatus;
use ratatui::layout::Rect;
use ratatui::text::{Line as TLine, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let current = state.current_file();
    let mut lines: Vec<TLine> = Vec::with_capacity(state.changeset.files.len() + 1);
    lines.push(TLine::styled(
        format!(" FILES ({})", state.changeset.files.len()),
        state.theme.sidebar_title,
    ));

    let height = usize::from(area.height).saturating_sub(1);
    let width = usize::from(area.width);
    for (idx, file) in state.changeset.files.iter().take(height).enumerate() {
        let selected = current == Some(idx);
        let marker = if selected { "\u{258c}" } else { " " };
        let glyph = match file.status {
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Modified => "M",
            FileStatus::Renamed => "R",
            FileStatus::Copied => "C",
        };
        let counts = format!(" +{} -{}", file.additions(), file.deletions());
        // Reserved columns for the staged dot and the viewed checkmark:
        // files stay aligned while the indicators light up in place.
        let path_room = width.saturating_sub(5 + counts.len());
        let path = truncate_left(&file.display_path(), path_room);
        let pad = path_room.saturating_sub(path.chars().count());
        let base = if selected {
            state.theme.sidebar_selected
        } else {
            state.theme.context
        };
        let staged = state.staged.as_ref().is_some_and(|s| s.is_staged(file));
        let viewed = state.is_viewed(idx);
        lines.push(TLine::from(vec![
            Span::styled(marker, base),
            Span::styled(
                if staged { "\u{25cf}" } else { " " },
                if staged {
                    state.theme.sidebar_staged
                } else {
                    base
                },
            ),
            Span::styled(
                if viewed { "\u{2713}" } else { " " },
                if viewed { state.theme.meta } else { base },
            ),
            Span::styled(format!("{glyph} {path}{}{counts}", " ".repeat(pad)), base),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// Keep the tail of a path — the informative part — when space is short.
fn truncate_left(path: &str, room: usize) -> String {
    let count = path.chars().count();
    if count <= room {
        return path.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let tail: String = path.chars().skip(count + 1 - room).collect();
    format!("\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    use super::truncate_left;

    #[test]
    fn truncation_keeps_the_tail() {
        assert_eq!(truncate_left("src/main.rs", 20), "src/main.rs");
        assert_eq!(
            truncate_left("crates/margin-core/src/patch.rs", 12),
            "\u{2026}rc/patch.rs"
        );
        assert_eq!(truncate_left("abc", 0), "");
    }
}
