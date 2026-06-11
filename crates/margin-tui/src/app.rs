//! The Elm core: state, messages, and the single `update` function.
//!
//! Every interaction is a [`Msg`]; [`update`] is the only place state
//! changes (ADR-0003). Side effects do not exist here — when reload/staging
//! arrive, they return as commands executed by the runtime shell.

use margin_core::{Changeset, LineKind};

/// One renderable/navigable row of the review stream. The changeset is
/// flattened into rows once; the cursor is an index into this vector, which
/// makes navigation, scrolling, and testing plain index arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    FileHeader {
        file: usize,
    },
    /// Stand-in body for files without hunks (binary, pure rename, mode-only).
    Meta {
        file: usize,
    },
    HunkHeader {
        file: usize,
        hunk: usize,
    },
    Line {
        file: usize,
        hunk: usize,
        line: usize,
        old_no: Option<u32>,
        new_no: Option<u32>,
    },
}

impl Row {
    pub fn file(&self) -> usize {
        match *self {
            Row::FileHeader { file }
            | Row::Meta { file }
            | Row::HunkHeader { file, .. }
            | Row::Line { file, .. } => file,
        }
    }
}

/// Every possible interaction. Keymaps translate key events into these;
/// tests drive the app with them directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    CursorDown,
    CursorUp,
    NextHunk,
    PrevHunk,
    NextFile,
    PrevFile,
    /// First `g` of the `gg` chord (second one jumps to top).
    GKey,
    Bottom,
    HalfPageDown,
    HalfPageUp,
    ToggleSidebar,
    ToggleHelp,
    Escape,
    Resize(u16, u16),
    Quit,
}

/// The single source of truth (ADR-0003).
pub struct AppState {
    pub changeset: Changeset,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub scroll: usize,
    pub sidebar_visible: bool,
    pub help_visible: bool,
    pub pending_g: bool,
    pub should_quit: bool,
    /// Terminal size; kept current by `Msg::Resize`.
    pub viewport: (u16, u16),
    pub theme: crate::theme::Theme,
}

impl AppState {
    pub fn new(changeset: Changeset) -> Self {
        let rows = build_rows(&changeset);
        Self {
            changeset,
            rows,
            cursor: 0,
            scroll: 0,
            sidebar_visible: true,
            help_visible: false,
            pending_g: false,
            should_quit: false,
            viewport: (80, 24),
            theme: crate::theme::Theme::default(),
        }
    }

    /// Index of the file the cursor is in, if any.
    pub fn current_file(&self) -> Option<usize> {
        self.rows.get(self.cursor).map(Row::file)
    }

    /// Rows visible in the main pane (viewport minus the status bar).
    pub fn content_height(&self) -> usize {
        usize::from(self.viewport.1.saturating_sub(1))
    }

    fn clamp_cursor(&mut self) {
        if self.cursor + 1 > self.rows.len() {
            self.cursor = self.rows.len().saturating_sub(1);
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let height = self.content_height().max(1);
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
    }

    fn jump<F>(&mut self, forward: bool, matches: F)
    where
        F: Fn(&Row) -> bool,
    {
        let found = if forward {
            self.rows
                .iter()
                .enumerate()
                .skip(self.cursor + 1)
                .find(|(_, r)| matches(r))
                .map(|(i, _)| i)
        } else {
            self.rows
                .iter()
                .enumerate()
                .take(self.cursor)
                .rev()
                .find(|(_, r)| matches(r))
                .map(|(i, _)| i)
        };
        if let Some(idx) = found {
            self.cursor = idx;
        }
    }
}

/// The only place state changes (ADR-0003).
pub fn update(state: &mut AppState, msg: Msg) {
    let pending_g = std::mem::take(&mut state.pending_g);

    match msg {
        Msg::CursorDown => {
            state.cursor = state.cursor.saturating_add(1);
            state.clamp_cursor();
        }
        Msg::CursorUp => state.cursor = state.cursor.saturating_sub(1),
        Msg::NextHunk => state.jump(true, |r| matches!(r, Row::HunkHeader { .. })),
        Msg::PrevHunk => state.jump(false, |r| matches!(r, Row::HunkHeader { .. })),
        Msg::NextFile => state.jump(true, |r| matches!(r, Row::FileHeader { .. })),
        Msg::PrevFile => state.jump(false, |r| matches!(r, Row::FileHeader { .. })),
        Msg::GKey => {
            if pending_g {
                state.cursor = 0;
            } else {
                state.pending_g = true;
            }
        }
        Msg::Bottom => state.cursor = state.rows.len().saturating_sub(1),
        Msg::HalfPageDown => {
            state.cursor = state.cursor.saturating_add(state.content_height() / 2);
            state.clamp_cursor();
        }
        Msg::HalfPageUp => {
            state.cursor = state.cursor.saturating_sub(state.content_height() / 2);
        }
        Msg::ToggleSidebar => state.sidebar_visible = !state.sidebar_visible,
        Msg::ToggleHelp => state.help_visible = !state.help_visible,
        Msg::Escape => state.help_visible = false,
        Msg::Resize(w, h) => state.viewport = (w, h),
        Msg::Quit => state.should_quit = true,
    }

    state.ensure_cursor_visible();
}

/// Flatten a changeset into the navigable review stream.
fn build_rows(changeset: &Changeset) -> Vec<Row> {
    let mut rows = Vec::new();
    for (file, diff) in changeset.files.iter().enumerate() {
        rows.push(Row::FileHeader { file });
        if diff.hunks.is_empty() {
            rows.push(Row::Meta { file });
        }
        for (hunk, h) in diff.hunks.iter().enumerate() {
            rows.push(Row::HunkHeader { file, hunk });
            let mut old_no = h.old_start;
            let mut new_no = h.new_start;
            for (line, l) in h.lines.iter().enumerate() {
                let (old, new) = match l.kind {
                    LineKind::Context => {
                        let pair = (Some(old_no), Some(new_no));
                        old_no += 1;
                        new_no += 1;
                        pair
                    }
                    LineKind::Deletion => {
                        let pair = (Some(old_no), None);
                        old_no += 1;
                        pair
                    }
                    LineKind::Addition => {
                        let pair = (None, Some(new_no));
                        new_no += 1;
                        pair
                    }
                };
                rows.push(Row::Line {
                    file,
                    hunk,
                    line,
                    old_no: old,
                    new_no: new,
                });
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use margin_core::parse_unified;

    fn sample() -> AppState {
        let patch = b"--- a.txt\n+++ b.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -10,1 +10,2 @@\n ten\n+eleven\n";
        AppState::new(parse_unified(patch).changeset)
    }

    #[test]
    fn rows_carry_line_numbers() {
        let state = sample();
        // FileHeader, HunkHeader, 4 lines, HunkHeader, 2 lines
        assert_eq!(state.rows.len(), 9);
        let Row::Line { old_no, new_no, .. } = state.rows[3] else {
            panic!("expected line row");
        };
        assert_eq!((old_no, new_no), (Some(2), None), "deletion: old side only");
        let Row::Line { old_no, new_no, .. } = state.rows[4] else {
            panic!("expected line row");
        };
        assert_eq!((old_no, new_no), (None, Some(2)), "addition: new side only");
    }

    #[test]
    fn gg_chord_jumps_to_top_and_g_alone_does_not() {
        let mut state = sample();
        update(&mut state, Msg::Bottom);
        assert_eq!(state.cursor, 8);
        update(&mut state, Msg::GKey);
        assert_eq!(state.cursor, 8, "single g must not move");
        update(&mut state, Msg::GKey);
        assert_eq!(state.cursor, 0, "gg jumps to top");

        update(&mut state, Msg::Bottom);
        update(&mut state, Msg::GKey);
        update(&mut state, Msg::CursorUp); // breaks the chord
        update(&mut state, Msg::GKey);
        assert_ne!(state.cursor, 0, "interrupted chord must not fire");
    }

    #[test]
    fn hunk_and_file_jumps() {
        let mut state = sample();
        update(&mut state, Msg::NextHunk);
        assert!(matches!(
            state.rows[state.cursor],
            Row::HunkHeader { hunk: 0, .. }
        ));
        update(&mut state, Msg::NextHunk);
        assert!(matches!(
            state.rows[state.cursor],
            Row::HunkHeader { hunk: 1, .. }
        ));
        update(&mut state, Msg::NextHunk); // no further hunk: stays put
        assert!(matches!(
            state.rows[state.cursor],
            Row::HunkHeader { hunk: 1, .. }
        ));
        update(&mut state, Msg::PrevFile);
        assert!(matches!(state.rows[state.cursor], Row::FileHeader { .. }));
    }

    #[test]
    fn scroll_follows_cursor() {
        let mut state = sample();
        update(&mut state, Msg::Resize(80, 5)); // content height 4
        update(&mut state, Msg::Bottom);
        assert_eq!(state.cursor, 8);
        assert_eq!(state.scroll, 5, "cursor at bottom of a 4-row window");
        update(&mut state, Msg::GKey);
        update(&mut state, Msg::GKey);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn empty_changeset_is_navigable_without_panic() {
        let mut state = AppState::new(Changeset::default());
        for msg in [
            Msg::CursorDown,
            Msg::CursorUp,
            Msg::NextHunk,
            Msg::PrevFile,
            Msg::Bottom,
            Msg::HalfPageDown,
        ] {
            update(&mut state, msg);
        }
        assert_eq!(state.cursor, 0);
        assert_eq!(state.current_file(), None);
    }
}
