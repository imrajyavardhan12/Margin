//! The `f` overlay: fuzzy jump-to-file.

use margin_core::FileStatus;
use ratatui::layout::Rect;
use ratatui::text::Line as TLine;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

pub fn render(state: &AppState, frame: &mut Frame, area: Rect) {
    let Some(picker) = &state.picker else { return };

    let width = u16::min(64, area.width.saturating_sub(4)).max(20);
    let height = u16::min(16, area.height.saturating_sub(2)).max(5);
    let popup = centered(area, width, height);
    let list_height = usize::from(height.saturating_sub(3));

    let mut lines: Vec<TLine> = Vec::with_capacity(list_height + 1);
    lines.push(TLine::styled(
        format!(" > {}\u{258c}", picker.query),
        state.theme.sidebar_selected,
    ));
    for (pos, &file_idx) in picker.filtered.iter().take(list_height).enumerate() {
        let Some(file) = state.changeset.files.get(file_idx) else {
            continue;
        };
        let selected = pos == picker.selected;
        let marker = if selected { "\u{258c}" } else { " " };
        let glyph = match file.status {
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Modified => "M",
            FileStatus::Renamed => "R",
            FileStatus::Copied => "C",
        };
        let style = if selected {
            state.theme.sidebar_selected
        } else {
            state.theme.context
        };
        lines.push(TLine::styled(
            format!("{marker}{glyph} {}", file.display_path()),
            style,
        ));
    }

    let title = format!(
        " jump to file ({}/{}) ",
        picker.filtered.len(),
        state.changeset.files.len()
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.help_border)
        .title(title);

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 3,
        width: u16::min(width, area.width),
        height: u16::min(height, area.height),
    }
}
