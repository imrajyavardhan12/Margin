//! The Elm core: state, messages, and the single `update` function.
//!
//! Every interaction is a [`Msg`]; [`update`] is the only place state
//! changes (ADR-0003). Side effects do not exist here — when reload/staging
//! arrive, they return as commands executed by the runtime shell.

use margin_core::{Changeset, Hunk, LineKind};

/// How the diff pane lays out changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Pick split or unified from the available width (the default).
    Auto,
    Unified,
    Split,
}

/// Main-pane width at which `Auto` switches to side-by-side.
const SPLIT_THRESHOLD: u16 = 120;

/// Computed pane geometry — the one place layout arithmetic lives, shared
/// by `update` (scrolling, auto layout) and `view` (rect splitting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panes {
    /// Sidebar width when visible at this terminal size.
    pub sidebar: Option<u16>,
    pub main_width: u16,
}

/// One renderable/navigable row of the review stream. The changeset is
/// flattened into rows once per layout; the cursor is an index into this
/// vector, which makes navigation, scrolling, and testing plain index
/// arithmetic.
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
    /// Unified layout: one diff line.
    Line {
        file: usize,
        hunk: usize,
        line: usize,
        old_no: Option<u32>,
        new_no: Option<u32>,
    },
    /// Split layout: one visual row pairing an old-side line with a
    /// new-side line. Each side is `(line index, line number)`.
    Split {
        file: usize,
        hunk: usize,
        left: Option<(usize, u32)>,
        right: Option<(usize, u32)>,
    },
}

impl Row {
    pub fn file(&self) -> usize {
        match *self {
            Row::FileHeader { file }
            | Row::Meta { file }
            | Row::HunkHeader { file, .. }
            | Row::Line { file, .. }
            | Row::Split { file, .. } => file,
        }
    }
}

/// `/` search over the changeset: incremental, smart-case regex.
pub struct SearchState {
    pub query: String,
    /// True while the user is typing in the search bar.
    pub typing: bool,
    /// Rows (indices into `AppState::rows`) with at least one match.
    pub matches: Vec<usize>,
    pub error: Option<String>,
    /// For highlighting visible rows (offset-correct against `printable`).
    pub regex: Option<regex::Regex>,
    /// For membership scans: matching raw bytes avoids a String allocation
    /// per row, which is what makes keystrokes on 250k-line diffs instant.
    bytes_regex: Option<regex::bytes::Regex>,
}

/// `f` fuzzy file picker.
pub struct PickerState {
    pub query: String,
    /// File indices passing the filter, best first.
    pub filtered: Vec<usize>,
    pub selected: usize,
}

/// Which surface receives keys; derived from state, used by the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Picker,
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
    /// `v`: switch unified/side-by-side, pinning over `Auto`.
    ToggleLayout,
    ToggleHelp,
    Escape,
    Resize(u16, u16),
    Quit,
    // Search (`/`, then `n`/`N`)
    SearchStart,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
    SearchCancel,
    NextMatch,
    PrevMatch,
    // Fuzzy file picker (`f`)
    PickerStart,
    PickerInput(char),
    PickerBackspace,
    PickerNext,
    PickerPrev,
    PickerConfirm,
    PickerCancel,
}

/// The single source of truth (ADR-0003).
pub struct AppState {
    pub changeset: Changeset,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub scroll: usize,
    pub sidebar_visible: bool,
    pub help_visible: bool,
    pub layout_mode: LayoutMode,
    /// Whether the current rows are split-layout rows (resolved from
    /// `layout_mode` and the pane width; kept in sync by `update`).
    pub split_active: bool,
    pub pending_g: bool,
    pub should_quit: bool,
    /// Terminal size; kept current by `Msg::Resize`.
    pub viewport: (u16, u16),
    pub theme: crate::theme::Theme,
    /// Memoizing, budgeted syntax/emphasis cache (ADR-0006).
    pub highlight: crate::highlight::HighlightCache,
    pub search: Option<SearchState>,
    pub picker: Option<PickerState>,
}

impl AppState {
    pub fn new(changeset: Changeset) -> Self {
        let mut state = Self {
            rows: Vec::new(),
            changeset,
            cursor: 0,
            scroll: 0,
            sidebar_visible: true,
            help_visible: false,
            layout_mode: LayoutMode::Auto,
            split_active: false,
            pending_g: false,
            should_quit: false,
            viewport: (80, 24),
            theme: crate::theme::Theme::default(),
            highlight: crate::highlight::HighlightCache::default(),
            search: None,
            picker: None,
        };
        state.rows = build_rows(&state.changeset, state.split_active);
        state.refresh_layout();
        state
    }

    /// Swap the visual theme, rebuilding the highlight cache so syntax
    /// colors come from the new theme (or disappear in degraded modes).
    pub fn apply_theme(&mut self, theme: crate::theme::Theme) {
        self.highlight = crate::highlight::HighlightCache::new(theme.syntax_theme);
        self.theme = theme;
    }

    /// Set the layout preference (from config/CLI) and re-resolve rows.
    pub fn set_layout_mode(&mut self, mode: LayoutMode) {
        self.layout_mode = mode;
        self.refresh_layout();
    }

    /// Pane geometry at the current viewport.
    pub fn panes(&self) -> Panes {
        let (width, _) = self.viewport;
        let sidebar = (self.sidebar_visible && width >= 60).then(|| u16::min(32, width / 3));
        Panes {
            sidebar,
            main_width: width - sidebar.unwrap_or(0),
        }
    }

    /// Index of the file the cursor is in, if any.
    pub fn current_file(&self) -> Option<usize> {
        self.rows.get(self.cursor).map(Row::file)
    }

    /// Which surface receives keys right now.
    pub fn input_mode(&self) -> InputMode {
        if self.picker.is_some() {
            InputMode::Picker
        } else if self.search.as_ref().is_some_and(|s| s.typing) {
            InputMode::Search
        } else {
            InputMode::Normal
        }
    }

    /// The active (confirmed or live) search regex, for match highlighting.
    pub fn search_regex(&self) -> Option<&regex::Regex> {
        self.search.as_ref().and_then(|s| s.regex.as_ref())
    }

    /// 1-based position of the cursor among matches, when it sits on one.
    pub fn match_position(&self) -> Option<(usize, usize)> {
        let search = self.search.as_ref()?;
        let total = search.matches.len();
        let pos = search.matches.iter().position(|&row| row == self.cursor)?;
        Some((pos + 1, total))
    }

    /// Rows visible in the main pane (viewport minus the status bar).
    pub fn content_height(&self) -> usize {
        usize::from(self.viewport.1.saturating_sub(1))
    }

    /// Re-resolve `split_active`; on change, rebuild the row stream and
    /// carry the cursor to the equivalent row in the new layout.
    fn refresh_layout(&mut self) {
        let split = match self.layout_mode {
            LayoutMode::Unified => false,
            LayoutMode::Split => true,
            LayoutMode::Auto => self.panes().main_width >= SPLIT_THRESHOLD,
        };
        if split == self.split_active {
            return;
        }
        let anchor = self.rows.get(self.cursor).copied();
        self.split_active = split;
        self.rows = build_rows(&self.changeset, split);
        self.cursor = anchor.map_or(0, |a| locate(&self.rows, a));
        self.clamp_cursor();
        // Match rows are layout-specific indices: recompute.
        if let Some(search) = &mut self.search {
            recompute_matches(search, &self.rows, &self.changeset);
        }
    }

    /// Move to the nearest match at/after the cursor (wrapping).
    fn jump_to_nearest_match(&mut self) {
        let Some(search) = &self.search else { return };
        let next = search
            .matches
            .iter()
            .find(|&&row| row >= self.cursor)
            .or_else(|| search.matches.first())
            .copied();
        if let Some(row) = next {
            self.cursor = row;
        }
    }

    fn jump_match(&mut self, forward: bool) {
        let Some(search) = &self.search else { return };
        let target = if forward {
            search
                .matches
                .iter()
                .find(|&&row| row > self.cursor)
                .or_else(|| search.matches.first())
        } else {
            search
                .matches
                .iter()
                .rev()
                .find(|&&row| row < self.cursor)
                .or_else(|| search.matches.last())
        }
        .copied();
        if let Some(row) = target {
            self.cursor = row;
        }
    }

    fn refilter_picker(&mut self) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let mut scored: Vec<(i64, usize)> = self
            .changeset
            .files
            .iter()
            .enumerate()
            .filter_map(|(idx, file)| {
                fuzzy_score(&picker.query, &file.display_path()).map(|score| (score, idx))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        picker.filtered = scored.into_iter().map(|(_, idx)| idx).collect();
        picker.selected = 0;
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
        Msg::ToggleSidebar => {
            state.sidebar_visible = !state.sidebar_visible;
            state.refresh_layout();
        }
        Msg::ToggleLayout => {
            // Pin the opposite of what is currently displayed; Auto is the
            // default until the user expresses a preference.
            state.layout_mode = if state.split_active {
                LayoutMode::Unified
            } else {
                LayoutMode::Split
            };
            state.refresh_layout();
        }
        Msg::ToggleHelp => state.help_visible = !state.help_visible,
        Msg::Escape => {
            if state.help_visible {
                state.help_visible = false;
            } else {
                state.search = None;
            }
        }
        Msg::Resize(w, h) => {
            state.viewport = (w, h);
            state.refresh_layout();
        }
        Msg::Quit => state.should_quit = true,

        Msg::SearchStart => {
            state.search = Some(SearchState {
                query: String::new(),
                typing: true,
                matches: Vec::new(),
                error: None,
                regex: None,
                bytes_regex: None,
            });
        }
        Msg::SearchInput(c) => {
            if let Some(search) = &mut state.search {
                search.query.push(c);
                recompute_matches(search, &state.rows, &state.changeset);
            }
            state.jump_to_nearest_match();
        }
        Msg::SearchBackspace => {
            if let Some(search) = &mut state.search {
                search.query.pop();
                recompute_matches(search, &state.rows, &state.changeset);
            }
        }
        Msg::SearchConfirm => {
            if let Some(search) = &mut state.search {
                if search.query.is_empty() {
                    state.search = None;
                } else {
                    search.typing = false;
                }
            }
        }
        Msg::SearchCancel => state.search = None,
        Msg::NextMatch => state.jump_match(true),
        Msg::PrevMatch => state.jump_match(false),

        Msg::PickerStart => {
            state.picker = Some(PickerState {
                query: String::new(),
                filtered: Vec::new(),
                selected: 0,
            });
            state.refilter_picker();
        }
        Msg::PickerInput(c) => {
            if state.picker.is_some() {
                if let Some(picker) = &mut state.picker {
                    picker.query.push(c);
                }
                state.refilter_picker();
            }
        }
        Msg::PickerBackspace => {
            if state.picker.is_some() {
                if let Some(picker) = &mut state.picker {
                    picker.query.pop();
                }
                state.refilter_picker();
            }
        }
        Msg::PickerNext => {
            if let Some(picker) = &mut state.picker {
                if !picker.filtered.is_empty() {
                    picker.selected = (picker.selected + 1) % picker.filtered.len();
                }
            }
        }
        Msg::PickerPrev => {
            if let Some(picker) = &mut state.picker {
                if !picker.filtered.is_empty() {
                    picker.selected =
                        (picker.selected + picker.filtered.len() - 1) % picker.filtered.len();
                }
            }
        }
        Msg::PickerConfirm => {
            if let Some(picker) = state.picker.take() {
                if let Some(&file) = picker.filtered.get(picker.selected) {
                    if let Some(row) = state
                        .rows
                        .iter()
                        .position(|r| matches!(r, Row::FileHeader { file: f } if *f == file))
                    {
                        state.cursor = row;
                    }
                }
            }
        }
        Msg::PickerCancel => state.picker = None,
    }

    state.ensure_cursor_visible();
}

/// Recompile the query (smart-case: case-insensitive unless it contains an
/// uppercase letter) and rescan the row stream.
fn recompute_matches(search: &mut SearchState, rows: &[Row], changeset: &Changeset) {
    search.matches.clear();
    search.regex = None;
    search.bytes_regex = None;
    search.error = None;
    if search.query.is_empty() {
        return;
    }
    let smart_case = !search.query.chars().any(char::is_uppercase);
    let compiled = regex::RegexBuilder::new(&search.query)
        .case_insensitive(smart_case)
        .size_limit(1 << 20)
        .build();
    let compiled_bytes = regex::bytes::RegexBuilder::new(&search.query)
        .case_insensitive(smart_case)
        .size_limit(1 << 20)
        .build();
    match (compiled, compiled_bytes) {
        (Ok(regex), Ok(bytes_regex)) => {
            search.matches = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row_matches(&bytes_regex, &regex, changeset, row))
                .map(|(idx, _)| idx)
                .collect();
            search.regex = Some(regex);
            search.bytes_regex = Some(bytes_regex);
        }
        _ => search.error = Some("invalid regex".to_string()),
    }
}

/// Does a row's text match? File paths and line contents count; hunk
/// headers and meta rows do not. Content is matched on raw bytes (no
/// allocation); the rare divergence from the printable form — patterns
/// targeting literal tabs or control bytes — is an accepted trade for
/// instant keystrokes on quarter-million-line diffs.
fn row_matches(
    bytes_regex: &regex::bytes::Regex,
    regex: &regex::Regex,
    changeset: &Changeset,
    row: &Row,
) -> bool {
    let line_matches = |file: usize, hunk: usize, line: usize| {
        changeset
            .files
            .get(file)
            .and_then(|f| f.hunks.get(hunk))
            .and_then(|h| h.lines.get(line))
            .is_some_and(|l| bytes_regex.is_match(&l.content))
    };
    match *row {
        Row::FileHeader { file } => changeset
            .files
            .get(file)
            .is_some_and(|f| regex.is_match(&f.display_path())),
        Row::Line {
            file, hunk, line, ..
        } => line_matches(file, hunk, line),
        Row::Split {
            file,
            hunk,
            left,
            right,
        } => [left, right]
            .into_iter()
            .flatten()
            .any(|(line, _)| line_matches(file, hunk, line)),
        Row::HunkHeader { .. } | Row::Meta { .. } => false,
    }
}

/// Dependency-free fuzzy match: query chars must appear in order
/// (case-insensitive). Higher is better; consecutive runs and early, tight
/// matches win. Empty queries match everything at score 0.
fn fuzzy_score(query: &str, target: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let target_lower: Vec<char> = target.to_lowercase().chars().collect();
    let mut score: i64 = 0;
    let mut pos = 0usize;
    let mut first_hit: Option<usize> = None;
    let mut previous_hit: Option<usize> = None;
    for qc in query.to_lowercase().chars() {
        let found = target_lower
            .iter()
            .enumerate()
            .skip(pos)
            .find(|(_, &tc)| tc == qc)
            .map(|(i, _)| i)?;
        if previous_hit == Some(found.wrapping_sub(1)) {
            score += 5; // consecutive run bonus
        }
        first_hit.get_or_insert(found);
        previous_hit = Some(found);
        pos = found + 1;
    }
    // Tighter spans and earlier starts read as better matches.
    let span = pos as i64 - first_hit.unwrap_or(0) as i64;
    score -= span;
    score -= first_hit.unwrap_or(0) as i64 / 2;
    Some(score)
}

/// Flatten a changeset into the navigable review stream for one layout.
fn build_rows(changeset: &Changeset, split: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for (file, diff) in changeset.files.iter().enumerate() {
        rows.push(Row::FileHeader { file });
        if diff.hunks.is_empty() {
            rows.push(Row::Meta { file });
        }
        for (hunk, h) in diff.hunks.iter().enumerate() {
            rows.push(Row::HunkHeader { file, hunk });
            if split {
                push_split_rows(&mut rows, file, hunk, h);
            } else {
                push_unified_rows(&mut rows, file, hunk, h);
            }
        }
    }
    rows
}

fn push_unified_rows(rows: &mut Vec<Row>, file: usize, hunk: usize, h: &Hunk) {
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

/// Side-by-side pairing: a run of deletions and the run of additions that
/// follows it are zipped row-by-row (the classic split-diff alignment);
/// context lines occupy both sides.
fn push_split_rows(rows: &mut Vec<Row>, file: usize, hunk: usize, h: &Hunk) {
    let mut old_no = h.old_start;
    let mut new_no = h.new_start;
    let mut dels: Vec<(usize, u32)> = Vec::new();
    let mut adds: Vec<(usize, u32)> = Vec::new();

    let flush =
        |rows: &mut Vec<Row>, dels: &mut Vec<(usize, u32)>, adds: &mut Vec<(usize, u32)>| {
            let height = usize::max(dels.len(), adds.len());
            for i in 0..height {
                rows.push(Row::Split {
                    file,
                    hunk,
                    left: dels.get(i).copied(),
                    right: adds.get(i).copied(),
                });
            }
            dels.clear();
            adds.clear();
        };

    for (line, l) in h.lines.iter().enumerate() {
        match l.kind {
            LineKind::Deletion => {
                dels.push((line, old_no));
                old_no += 1;
            }
            LineKind::Addition => {
                adds.push((line, new_no));
                new_no += 1;
            }
            LineKind::Context => {
                flush(rows, &mut dels, &mut adds);
                rows.push(Row::Split {
                    file,
                    hunk,
                    left: Some((line, old_no)),
                    right: Some((line, new_no)),
                });
                old_no += 1;
                new_no += 1;
            }
        }
    }
    flush(rows, &mut dels, &mut adds);
}

/// Find the row in a (re-built) stream that best matches an anchor row from
/// the previous layout, so toggling layouts keeps the user's place.
fn locate(rows: &[Row], anchor: Row) -> usize {
    let target = |row: &Row| match (anchor, *row) {
        (Row::FileHeader { file: a } | Row::Meta { file: a }, Row::FileHeader { file: b }) => {
            a == b
        }
        (Row::HunkHeader { file: af, hunk: ah }, Row::HunkHeader { file: bf, hunk: bh }) => {
            af == bf && ah == bh
        }
        // A unified line maps to the split row containing it, and vice versa.
        (
            Row::Line {
                file: af,
                hunk: ah,
                line,
                ..
            },
            Row::Split {
                file: bf,
                hunk: bh,
                left,
                right,
            },
        ) => {
            af == bf
                && ah == bh
                && (left.is_some_and(|(l, _)| l == line) || right.is_some_and(|(r, _)| r == line))
        }
        (
            Row::Split {
                file: af,
                hunk: ah,
                left,
                right,
            },
            Row::Line {
                file: bf,
                hunk: bh,
                line,
                ..
            },
        ) => {
            af == bf
                && ah == bh
                && (left.is_some_and(|(l, _)| l == line) || right.is_some_and(|(r, _)| r == line))
        }
        _ => false,
    };
    rows.iter()
        .position(target)
        .or_else(|| {
            // Fall back to the row's hunk header, then its file header.
            rows.iter().position(|row| match (anchor, *row) {
                (
                    Row::Line { file, hunk, .. } | Row::Split { file, hunk, .. },
                    Row::HunkHeader { file: bf, hunk: bh },
                ) => file == bf && hunk == bh,
                _ => false,
            })
        })
        .or_else(|| {
            rows.iter()
                .position(|row| matches!(row, Row::FileHeader { file } if *file == anchor.file()))
        })
        .unwrap_or(0)
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
            Msg::ToggleLayout,
        ] {
            update(&mut state, msg);
        }
        assert_eq!(state.cursor, 0);
        assert_eq!(state.current_file(), None);
    }

    #[test]
    fn auto_layout_switches_on_width() {
        let mut state = sample();
        update(&mut state, Msg::Resize(80, 24));
        assert!(!state.split_active, "80 cols: unified");
        update(&mut state, Msg::Resize(200, 24));
        assert!(state.split_active, "200 cols (main 168): split");
        update(&mut state, Msg::Resize(80, 24));
        assert!(!state.split_active, "auto flips back");
    }

    #[test]
    fn split_rows_pair_deletions_with_additions() {
        let mut state = sample();
        update(&mut state, Msg::Resize(200, 24));
        // Hunk 1: ctx / (del+add paired) / ctx -> 3 split rows.
        let split_rows: Vec<Row> = state
            .rows
            .iter()
            .copied()
            .filter(|r| matches!(r, Row::Split { hunk: 0, .. }))
            .collect();
        assert_eq!(split_rows.len(), 3);
        let Row::Split { left, right, .. } = split_rows[1] else {
            panic!("expected split row");
        };
        assert_eq!(left.map(|(_, no)| no), Some(2), "old line 2 on the left");
        assert_eq!(right.map(|(_, no)| no), Some(2), "new line 2 on the right");
        // Hunk 2: addition with no paired deletion -> blank left side.
        let Some(Row::Split { left, right, .. }) = state.rows.iter().copied().find(|r| {
            matches!(
                r,
                Row::Split {
                    hunk: 1,
                    left: None,
                    ..
                }
            )
        }) else {
            panic!("expected an addition-only split row");
        };
        assert_eq!(left, None);
        assert_eq!(right.map(|(_, no)| no), Some(11));
    }

    fn type_search(state: &mut AppState, query: &str) {
        update(state, Msg::SearchStart);
        for c in query.chars() {
            update(state, Msg::SearchInput(c));
        }
    }

    #[test]
    fn search_finds_jumps_and_wraps() {
        let mut state = sample(); // lines: one/-two/+TWO/three + ten/+eleven
        type_search(&mut state, "t");
        // smart-case 't' matches two, TWO, three, ten, eleven... jump landed
        // on the first matching row at/after the old cursor (row 0 -> first match).
        assert!(state.search.as_ref().is_some_and(|s| !s.matches.is_empty()));
        update(&mut state, Msg::SearchConfirm);
        assert_eq!(state.input_mode(), InputMode::Normal);

        let first = state.cursor;
        update(&mut state, Msg::NextMatch);
        assert!(state.cursor > first, "n advances");
        update(&mut state, Msg::Bottom);
        update(&mut state, Msg::NextMatch);
        assert_eq!(state.cursor, first, "n wraps to the first match");
        update(&mut state, Msg::PrevMatch);
        let last = state.cursor;
        assert!(last > first, "N wraps backward to the last match");

        update(&mut state, Msg::Escape);
        assert!(state.search.is_none(), "Esc clears the finished search");
    }

    #[test]
    fn search_is_smart_case_and_reports_bad_regexes() {
        let mut state = sample();
        type_search(&mut state, "two");
        let lower = state.search.as_ref().map(|s| s.matches.len()).unwrap_or(0);
        assert_eq!(lower, 2, "lowercase matches two and TWO");

        update(&mut state, Msg::SearchCancel);
        type_search(&mut state, "TWO");
        let upper = state.search.as_ref().map(|s| s.matches.len()).unwrap_or(0);
        assert_eq!(upper, 1, "uppercase in query switches to case-sensitive");

        update(&mut state, Msg::SearchCancel);
        type_search(&mut state, "[");
        let search = state.search.as_ref().unwrap_or_else(|| panic!("search"));
        assert!(
            search.error.is_some(),
            "invalid regex reports, never panics"
        );
        assert!(search.matches.is_empty());
    }

    #[test]
    fn picker_filters_and_jumps_to_file_header() {
        let patch = b"--- a/src/parser.rs\n+++ b/src/parser.rs\n@@ -1,1 +1,1 @@\n-a\n+b\n\
--- a/docs/notes.md\n+++ b/docs/notes.md\n@@ -1,1 +1,1 @@\n-c\n+d\n";
        let mut state = AppState::new(parse_unified(patch).changeset);
        update(&mut state, Msg::Bottom);
        update(&mut state, Msg::PickerStart);
        assert_eq!(state.input_mode(), InputMode::Picker);
        assert_eq!(
            state.picker.as_ref().map(|p| p.filtered.len()),
            Some(2),
            "empty query lists every file"
        );
        for c in "notes".chars() {
            update(&mut state, Msg::PickerInput(c));
        }
        assert_eq!(state.picker.as_ref().map(|p| p.filtered.len()), Some(1));
        update(&mut state, Msg::PickerConfirm);
        assert!(state.picker.is_none());
        assert!(
            matches!(state.rows[state.cursor], Row::FileHeader { file: 1 }),
            "cursor lands on the picked file's header"
        );
    }

    #[test]
    fn fuzzy_score_prefers_tight_early_matches() {
        assert!(fuzzy_score("xyz", "margin-core").is_none());
        assert!(fuzzy_score("", "anything") == Some(0));
        let tight = fuzzy_score("parse", "src/parser.rs");
        let loose = fuzzy_score("parse", "p/a/r/s/e_far_apart.rs");
        assert!(tight > loose, "{tight:?} vs {loose:?}");
        let early = fuzzy_score("app", "src/app.rs");
        let late = fuzzy_score("app", "tests/snapshots/app.snap");
        assert!(early > late, "{early:?} vs {late:?}");
    }

    #[test]
    fn toggle_layout_pins_and_remaps_cursor() {
        let mut state = sample();
        update(&mut state, Msg::Resize(80, 24));
        // Put the cursor on the addition line ("TWO", new line 2).
        while !matches!(state.rows[state.cursor], Row::Line { line: 2, .. }) {
            update(&mut state, Msg::CursorDown);
        }
        update(&mut state, Msg::ToggleLayout);
        assert!(state.split_active, "v pins split even at 80 cols");
        assert_eq!(state.layout_mode, LayoutMode::Split);
        let Row::Split { right, .. } = state.rows[state.cursor] else {
            panic!(
                "cursor should land on a split row, got {:?}",
                state.rows[state.cursor]
            );
        };
        assert_eq!(
            right.map(|(line, _)| line),
            Some(2),
            "cursor follows the same underlying line across layouts"
        );
        update(&mut state, Msg::ToggleLayout);
        assert!(!state.split_active);
        assert_eq!(state.layout_mode, LayoutMode::Unified);
        assert!(
            matches!(
                state.rows[state.cursor],
                Row::Line { line: 1, .. } | Row::Line { line: 2, .. }
            ),
            "cursor stays on the paired lines"
        );
    }
}
