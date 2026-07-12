//! The Elm core: state, messages, and the single `update` function.
//!
//! Every interaction is a [`Msg`]; [`update`] is the only place state
//! changes (ADR-0003). Side effects do not exist here — when reload/staging
//! arrive, they return as commands executed by the runtime shell.

use margin_core::{
    render_hunk_patch, render_reversed_hunk_patch, Changeset, FileDiff, Hunk, LineKind,
    RenderRefusal,
};

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

/// `x` typed-confirmation prompt: the rendered patches wait here while the
/// user types. Nothing is destroyed until `yes` + Enter (ADR-0014).
pub struct ConfirmState {
    /// What the user has typed so far.
    pub input: String,
    /// The file named in the prompt.
    pub label: String,
    /// Forward patch — the trash copy, persisted before the apply.
    pub backup: Vec<u8>,
    /// Reversed patch — what the executor applies to the working tree.
    pub patch: Vec<u8>,
}

/// Which surface receives keys; derived from state, used by the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Picker,
    Confirm,
    /// After `z`: the next key resolves the fold chord (`za`/`zA`).
    Fold,
}

/// Which write the selected hunk should request. `Stage`/`Unstage` are
/// index writes (ADR-0013); `Discard` is the working-tree write behind
/// the typed confirmation (ADR-0014).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkAction {
    Stage,
    Unstage,
    Discard,
}

impl HunkAction {
    pub fn past_tense(self) -> &'static str {
        match self {
            HunkAction::Stage => "staged",
            HunkAction::Unstage => "unstaged",
            HunkAction::Discard => "discarded",
        }
    }
}

/// Which paths currently have content staged in the index (index vs HEAD).
///
/// The sidebar's staged indicator reads this. It is pure data: the runtime
/// shell loads the `GitStaged` diff and hands the reduced set in, so
/// margin-tui never touches git (the same inversion as `DiffSource`,
/// ADR-0005). Matching is by the file's canonical byte path, so a file
/// modified in both the worktree and the index lines up across the two
/// diffs.
///
/// Carried as `Option<StagedFiles>` everywhere: `Some` means the summary
/// is authoritative (a worktree review, where staged-vs-not is real
/// information), `None` means not applicable (`--staged` reviews are
/// staged by definition; patches and ranges have no index at all). The
/// distinction matters — an empty `Some` refuses unstaging, `None` must
/// not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StagedFiles {
    paths: std::collections::HashSet<Vec<u8>>,
}

impl StagedFiles {
    /// Reduce an index-vs-HEAD changeset to the set of staged paths.
    pub fn from_staged_changeset(changeset: &Changeset) -> Self {
        Self {
            paths: changeset
                .files
                .iter()
                .filter_map(path_key)
                .map(<[u8]>::to_vec)
                .collect(),
        }
    }

    /// Whether this file (from the review) has staged content.
    pub fn is_staged(&self, file: &FileDiff) -> bool {
        path_key(file).is_some_and(|key| self.paths.contains(key))
    }
}

/// The byte path that identifies a file across diffs and reloads: the new
/// side, falling back to the old (deleted files) — mirroring
/// `FileDiff::display_path`'s choice, but on raw bytes so the match is
/// exact. Keys the staged summary and the fold (collapse) state.
fn path_key(file: &FileDiff) -> Option<&[u8]> {
    file.new_path.as_deref().or(file.old_path.as_deref())
}

/// Data-only effects returned by [`update`]; the runtime executes them
/// (ADR-0003: no I/O in the core, ADR-0013: writes are explicit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Apply the pre-rendered single-hunk patch to the index
    /// (`action` is `Stage` or `Unstage`).
    ApplyHunk { action: HunkAction, patch: Vec<u8> },
    /// Discard from the working tree (ADR-0014): persist `backup` to the
    /// trash first, then apply the reversed `patch`. Only ever produced
    /// by a confirmed `Msg::ConfirmSubmit`.
    DiscardHunk { backup: Vec<u8>, patch: Vec<u8> },
    /// Re-read the changeset from the active source (`r`).
    Reload,
    /// Persist the viewed marks (issue #20): lossy path → content digest.
    /// Sources without a stable identity (pager/patch) ignore this — the
    /// marks stay session-only.
    SaveViewed { entries: Vec<(String, u64)> },
}

/// Outcome of a command, fed back into [`update`] as
/// [`Msg::CommandFinished`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// The write succeeded; here is the reloaded changeset and the
    /// refreshed staged-files summary for the sidebar.
    Applied {
        action: HunkAction,
        changeset: Changeset,
        staged: Option<StagedFiles>,
    },
    /// `Command::Reload` succeeded.
    Reloaded {
        changeset: Changeset,
        staged: Option<StagedFiles>,
    },
    /// `Command::DiscardHunk` succeeded; `backed_up` says whether a trash
    /// entry exists (`discard_trash = false` disables them).
    Discarded {
        changeset: Changeset,
        staged: Option<StagedFiles>,
        backed_up: bool,
    },
    /// The hunk didn't apply. Carries the attempted action because the
    /// honest diagnosis differs: a stage that fails is usually already
    /// staged; an unstage that fails usually wasn't; a discard that
    /// fails means the file moved since load.
    Stale(HunkAction),
    /// The active source cannot do this (patch file, stdin, two-files).
    Unsupported(&'static str),
    /// The command completed with nothing to report (persistence).
    Done,
    Failed(String),
}

/// Effect boundary for the runtime shell (same dependency inversion as
/// `DiffSource`): the binary implements this over margin-vcs, so
/// margin-tui never depends on it.
pub trait CommandExecutor {
    fn execute(&mut self, command: Command) -> CommandResult;
}

/// Every possible interaction. Keymaps translate key events into these;
/// tests drive the app with them directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// `s` / `u`: request an index write for the hunk under the cursor.
    StageHunk,
    UnstageHunk,
    /// `x`: open the typed-confirmation prompt for discarding the hunk
    /// under the cursor (ADR-0014). The write happens only on
    /// `ConfirmSubmit` with `yes` typed.
    DiscardHunk,
    ConfirmInput(char),
    ConfirmBackspace,
    ConfirmSubmit,
    ConfirmCancel,
    /// `r`: re-read the changeset from the source.
    Reload,
    /// `m`: toggle the cursor's file viewed (marks + folds it).
    ToggleViewed,
    /// First key of the `za`/`zA` fold chord.
    ZKey,
    /// `za`: toggle the cursor's file collapsed.
    ToggleFold,
    /// `zA`: collapse everything, or expand everything when nothing is.
    ToggleFoldAll,
    /// Any other key after `z`: break the chord.
    FoldCancel,
    /// The runtime reporting a command's outcome.
    CommandFinished(CommandResult),
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
    /// `w`: wrap long lines instead of clipping.
    ToggleWrap,
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
    /// `w`: wrap long lines instead of clipping. Rows become 1..N screen
    /// lines tall; all scroll math goes through `row_height`.
    pub wrap: bool,
    pub pending_g: bool,
    pub should_quit: bool,
    /// Terminal size; kept current by `Msg::Resize`.
    pub viewport: (u16, u16),
    pub theme: crate::theme::Theme,
    /// Memoizing, budgeted syntax/emphasis cache (ADR-0006).
    pub highlight: crate::highlight::HighlightCache,
    /// One-shot feedback line (command outcomes, refusals); cleared by
    /// the next keypress.
    pub status_message: Option<String>,
    /// Which files have staged content, for the sidebar indicator. Set by
    /// the runtime shell at startup and refreshed after each index write.
    /// `None` when the summary does not apply (`--staged` reviews, patches,
    /// ranges) — see [`StagedFiles`] for why that differs from empty.
    pub staged: Option<StagedFiles>,
    pub search: Option<SearchState>,
    pub picker: Option<PickerState>,
    /// `x` typed-confirmation prompt; `Some` while awaiting the word.
    pub confirm: Option<ConfirmState>,
    /// Watch mode (`-w`): the status bar shows `[watch]` and the runtime
    /// feeds debounced reloads. Set by the binary at startup.
    pub watching: bool,
    /// Fold (collapse) state per file, keyed by canonical byte path so it
    /// survives reloads (issue #21). Every current file has an entry.
    fold: std::collections::HashMap<Vec<u8>, bool>,
    /// Viewed marks (issue #20): path → content digest at mark time. A
    /// mark only counts while the digest still matches — a changed file
    /// un-views itself. Loaded by the binary from the per-DiffId store;
    /// every toggle emits `Command::SaveViewed`.
    viewed: std::collections::HashMap<Vec<u8>, u64>,
    /// User/repo `collapse` globs; combined with the built-in heuristics
    /// they decide the default fold for files not seen before.
    collapse_globs: Vec<String>,
    /// True between `z` and the chord's second key.
    pub pending_z: bool,
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
            wrap: false,
            pending_g: false,
            should_quit: false,
            viewport: (80, 24),
            theme: crate::theme::Theme::default(),
            highlight: crate::highlight::HighlightCache::default(),
            status_message: None,
            staged: None,
            search: None,
            picker: None,
            confirm: None,
            watching: false,
            fold: std::collections::HashMap::new(),
            viewed: std::collections::HashMap::new(),
            collapse_globs: Vec::new(),
            pending_z: false,
        };
        state.reset_folds();
        let flags = state.collapsed_flags();
        state.rows = build_rows(&state.changeset, state.split_active, &flags);
        state.refresh_layout();
        state
    }

    /// Set the `collapse` globs (from config) and re-derive every file's
    /// default fold. Called by the binary at startup, before interaction.
    pub fn set_collapse_globs(&mut self, globs: Vec<String>) {
        self.collapse_globs = globs;
        self.reset_folds();
        let file = self.current_file();
        self.rebuild_rows_after_fold(file);
    }

    /// Default fold for a file: built-in generated-file heuristics plus
    /// the configured globs (issue #21).
    fn auto_collapsed(&self, file: &FileDiff) -> bool {
        let Some(key) = path_key(file) else {
            return false;
        };
        margin_core::is_generated(key)
            || self
                .collapse_globs
                .iter()
                .any(|glob| margin_core::glob_match(glob, key))
    }

    /// Re-derive every file's fold from the defaults (startup/config).
    fn reset_folds(&mut self) {
        let fold = self
            .changeset
            .files
            .iter()
            .filter_map(|f| path_key(f).map(|k| (k.to_vec(), self.auto_collapsed(f))))
            .collect();
        self.fold = fold;
    }

    /// Whether this file renders header-only right now.
    pub fn is_collapsed(&self, file: usize) -> bool {
        self.changeset
            .files
            .get(file)
            .and_then(path_key)
            .is_some_and(|k| self.fold.get(k).copied().unwrap_or(false))
    }

    fn collapsed_flags(&self) -> Vec<bool> {
        (0..self.changeset.files.len())
            .map(|i| self.is_collapsed(i))
            .collect()
    }

    /// Seed viewed marks from the persisted store (binary startup): only
    /// entries whose digest still matches the current content count, and
    /// those files start folded — collapse what you've already reviewed.
    pub fn set_viewed(&mut self, entries: impl IntoIterator<Item = (Vec<u8>, u64)>) {
        let stored: std::collections::HashMap<Vec<u8>, u64> = entries.into_iter().collect();
        let valid: Vec<(Vec<u8>, u64)> = self
            .changeset
            .files
            .iter()
            .filter_map(|file| {
                let key = path_key(file)?;
                let digest = margin_core::file_digest(file);
                (stored.get(key).copied() == Some(digest)).then(|| (key.to_vec(), digest))
            })
            .collect();
        self.viewed.clear();
        for (key, digest) in valid {
            self.fold.insert(key.clone(), true);
            self.viewed.insert(key, digest);
        }
        let file = self.current_file();
        self.rebuild_rows_after_fold(file);
    }

    /// Whether this file is marked viewed. O(1): entries are validated
    /// against content digests at load/toggle/reload time, never here —
    /// the sidebar calls this per row per frame.
    pub fn is_viewed(&self, file: usize) -> bool {
        self.changeset
            .files
            .get(file)
            .and_then(path_key)
            .is_some_and(|key| self.viewed.contains_key(key))
    }

    /// Snapshot the marks for the persistence command (lossy paths: the
    /// store is advisory, digests do the real matching). Sorted, so the
    /// store file is deterministic.
    fn save_viewed_command(&self) -> Command {
        let mut entries: Vec<(String, u64)> = self
            .viewed
            .iter()
            .map(|(path, digest)| (String::from_utf8_lossy(path).into_owned(), *digest))
            .collect();
        entries.sort();
        Command::SaveViewed { entries }
    }

    /// Rebuild rows after fold changes, keeping the cursor on `file`'s
    /// header — the body it may have been in can vanish.
    fn rebuild_rows_after_fold(&mut self, file: Option<usize>) {
        let flags = self.collapsed_flags();
        self.rows = build_rows(&self.changeset, self.split_active, &flags);
        if let Some(file) = file {
            self.cursor = self
                .rows
                .iter()
                .position(|r| matches!(r, Row::FileHeader { file: f } if *f == file))
                .unwrap_or(0);
        }
        self.clamp_cursor();
        // Match rows are indices into the rebuilt stream: recompute.
        if let Some(search) = &mut self.search {
            recompute_matches(search, &self.rows, &self.changeset);
        }
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
        if self.confirm.is_some() {
            InputMode::Confirm
        } else if self.picker.is_some() {
            InputMode::Picker
        } else if self.search.as_ref().is_some_and(|s| s.typing) {
            InputMode::Search
        } else if self.pending_z {
            InputMode::Fold
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
        let flags = self.collapsed_flags();
        self.rows = build_rows(&self.changeset, split, &flags);
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

    /// Visual height of a row at the current pane width. 1 unless wrapping;
    /// the renderer and this function must agree, so both defer to
    /// `view::wrap` (see the warning there about ceil-division).
    pub fn row_height(&self, idx: usize) -> usize {
        if !self.wrap {
            return 1;
        }
        let main_width = usize::from(self.panes().main_width);
        match self.rows.get(idx) {
            Some(&Row::Line {
                file, hunk, line, ..
            }) => {
                let budget = main_width.saturating_sub(crate::view::UNIFIED_PREFIX_COLS);
                self.line_wrap_count(file, hunk, line, budget)
            }
            Some(&Row::Split {
                file,
                hunk,
                left,
                right,
            }) => {
                let (left_w, right_w) = crate::view::split_content_widths(main_width);
                let l = left.map_or(1, |(line, _)| {
                    self.line_wrap_count(file, hunk, line, left_w)
                });
                let r = right.map_or(1, |(line, _)| {
                    self.line_wrap_count(file, hunk, line, right_w)
                });
                l.max(r)
            }
            // Exhaustive on purpose: a new Row variant must decide its
            // height here or scroll math silently treats it as one line.
            Some(Row::FileHeader { .. } | Row::Meta { .. } | Row::HunkHeader { .. }) | None => 1,
        }
    }

    fn line_wrap_count(&self, file: usize, hunk: usize, line: usize, budget: usize) -> usize {
        let Some(l) = self
            .changeset
            .files
            .get(file)
            .and_then(|f| f.hunks.get(hunk))
            .and_then(|h| h.lines.get(line))
        else {
            return 1;
        };
        // Must measure exactly the text `composed_line_spans` renders:
        // printable content plus the shared no-newline suffix.
        let mut text = crate::view::printable(&l.content);
        if l.no_newline {
            text.push_str(crate::view::NO_NEWLINE_SUFFIX);
        }
        // Heights are only ever compared against the viewport, so counting
        // can saturate one past it.
        crate::view::wrap_count(&text, budget, self.content_height().max(1) + 1)
    }

    /// The (file, hunk) indices addressed by the cursor row, if any —
    /// shared by every per-hunk action (`s`/`u`/`x`).
    fn hunk_under_cursor(&self) -> Option<(usize, usize)> {
        match self.rows.get(self.cursor) {
            Some(
                &Row::HunkHeader { file, hunk }
                | &Row::Line { file, hunk, .. }
                | &Row::Split { file, hunk, .. },
            ) => Some((file, hunk)),
            _ => None,
        }
    }

    /// 1-based `(hunk, of)` for the cursor within its file — reviewers
    /// think in hunks, so the status bar shows this (issue #19).
    pub fn hunk_position(&self) -> Option<(usize, usize)> {
        let (file, hunk) = self.hunk_under_cursor()?;
        let total = self.changeset.files.get(file)?.hunks.len();
        Some((hunk + 1, total))
    }

    /// Build the index-write command for the hunk under the cursor, or
    /// explain in the status bar why there isn't one (ADR-0013: refusals
    /// are messages, not errors).
    fn request_hunk_apply(&mut self, action: HunkAction) -> Option<Command> {
        let located = self.hunk_under_cursor();
        let located = located.and_then(|(file, hunk)| {
            let file = self.changeset.files.get(file)?;
            Some((file, file.hunks.get(hunk)?))
        });
        let Some((file, hunk)) = located else {
            self.status_message = Some("no hunk under the cursor".into());
            return None;
        };
        // When the staged summary is authoritative, an unstage on a file
        // with no index content can only fail — refuse it accurately here
        // instead of letting the apply report a misleading failure.
        if action == HunkAction::Unstage {
            if let Some(staged) = &self.staged {
                if !staged.is_staged(file) {
                    self.status_message = Some("nothing staged in this file".into());
                    return None;
                }
            }
        }
        let rendered = match action {
            HunkAction::Stage => render_hunk_patch(file, hunk),
            HunkAction::Unstage => render_reversed_hunk_patch(file, hunk),
            // Discards flow through request_discard (typed confirm,
            // ADR-0014); this function only builds index writes.
            HunkAction::Discard => return None,
        };
        match rendered {
            Ok(patch) => Some(Command::ApplyHunk { action, patch }),
            Err(refusal) => {
                self.status_message = Some(
                    match refusal {
                        RenderRefusal::Binary => "cannot stage binary files",
                        RenderRefusal::Rename => "cannot stage renames yet",
                        RenderRefusal::UnsafePath => "cannot stage: path needs git quoting",
                    }
                    .into(),
                );
                None
            }
        }
    }

    /// `x`: open the typed-confirmation prompt for the hunk under the
    /// cursor, with both patches pre-rendered so refusals surface now —
    /// before the user is asked to type anything (ADR-0014). No command
    /// is issued here; only `ConfirmSubmit` with `yes` typed does that.
    fn request_discard(&mut self) {
        let located = self.hunk_under_cursor();
        let located = located.and_then(|(file, hunk)| {
            let file = self.changeset.files.get(file)?;
            Some((file, file.hunks.get(hunk)?))
        });
        let Some((file, hunk)) = located else {
            self.status_message = Some("no hunk under the cursor".into());
            return;
        };
        let rendered = render_hunk_patch(file, hunk)
            .and_then(|backup| render_reversed_hunk_patch(file, hunk).map(|patch| (backup, patch)));
        match rendered {
            Ok((backup, patch)) => {
                self.confirm = Some(ConfirmState {
                    input: String::new(),
                    label: file.display_path().into_owned(),
                    backup,
                    patch,
                });
            }
            Err(refusal) => {
                self.status_message = Some(
                    match refusal {
                        RenderRefusal::Binary => "cannot discard binary files",
                        RenderRefusal::Rename => "cannot discard renames yet",
                        RenderRefusal::UnsafePath => "cannot discard: path needs git quoting",
                    }
                    .into(),
                );
            }
        }
    }

    /// Absorb a command outcome: swap in the fresh changeset and re-anchor
    /// on success, report otherwise.
    fn finish_command(&mut self, result: CommandResult) {
        match result {
            CommandResult::Applied {
                action,
                changeset,
                staged,
            } => {
                self.absorb_changeset(changeset, staged);
                self.status_message = Some(format!("hunk {}", action.past_tense()));
            }
            CommandResult::Reloaded { changeset, staged } => {
                self.absorb_changeset(changeset, staged);
                self.status_message = Some("reloaded".into());
            }
            CommandResult::Discarded {
                changeset,
                staged,
                backed_up,
            } => {
                self.absorb_changeset(changeset, staged);
                self.status_message = Some(if backed_up {
                    "hunk discarded — `margin undo` restores it".into()
                } else {
                    "hunk discarded (backup disabled)".into()
                });
            }
            // The apply's dry run refused. The likeliest cause depends on
            // the direction: re-staging what's already in the index, or
            // unstaging what never was. Either way `r` shows the truth.
            CommandResult::Stale(HunkAction::Stage) => {
                self.status_message = Some(
                    "hunk didn't apply — already staged, or changed since load (r reloads)".into(),
                );
            }
            CommandResult::Stale(HunkAction::Unstage) => {
                self.status_message =
                    Some("hunk isn't staged — or it changed since load (r reloads)".into());
            }
            CommandResult::Stale(HunkAction::Discard) => {
                self.status_message =
                    Some("hunk didn't apply — the file changed since load (r reloads)".into());
            }
            CommandResult::Unsupported(why) => self.status_message = Some(why.into()),
            CommandResult::Done => {}
            CommandResult::Failed(err) => self.status_message = Some(format!("failed: {err}")),
        }
    }

    /// Swap in a freshly loaded changeset, keeping the user's place: rebuild
    /// rows, re-anchor the cursor via `locate`, rebuild the index-keyed
    /// highlight cache, and recompute search matches.
    fn absorb_changeset(&mut self, changeset: Changeset, staged: Option<StagedFiles>) {
        let anchor = self.rows.get(self.cursor).copied();
        self.changeset = changeset;
        self.staged = staged;
        // Viewed marks only survive byte-identical content: one digest per
        // current file, then drop every mark that no longer matches. An
        // invalidated file's fold entry goes too, so it reopens (or falls
        // back to the auto-collapse default) instead of hiding new changes.
        let current: std::collections::HashMap<Vec<u8>, u64> = self
            .changeset
            .files
            .iter()
            .filter_map(|f| path_key(f).map(|k| (k.to_vec(), margin_core::file_digest(f))))
            .collect();
        let stale: Vec<Vec<u8>> = self
            .viewed
            .iter()
            .filter(|(path, digest)| current.get(*path) != Some(digest))
            .map(|(path, _)| path.clone())
            .collect();
        for path in stale {
            self.viewed.remove(&path);
            self.fold.remove(&path);
        }
        // Fold state survives the reload by path: a file the user expanded
        // stays expanded; files appearing for the first time get defaults.
        let fold = self
            .changeset
            .files
            .iter()
            .filter_map(|f| {
                path_key(f).map(|k| {
                    let collapsed = self
                        .fold
                        .get(k)
                        .copied()
                        .unwrap_or_else(|| self.auto_collapsed(f));
                    (k.to_vec(), collapsed)
                })
            })
            .collect();
        self.fold = fold;
        let flags = self.collapsed_flags();
        self.rows = build_rows(&self.changeset, self.split_active, &flags);
        self.cursor = anchor.map_or(0, |a| locate(&self.rows, a));
        self.clamp_cursor();
        self.highlight = crate::highlight::HighlightCache::new(self.theme.syntax_theme);
        if let Some(search) = &mut self.search {
            recompute_matches(search, &self.rows, &self.changeset);
        }
        // The picker's filtered list holds file *indices*: stale against a
        // reloaded changeset they would jump to whatever file now occupies
        // the old position. Refilter against the new files.
        if self.picker.is_some() {
            self.refilter_picker();
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let height = self.content_height().max(1);
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
            return;
        }
        // Fill the viewport upward from the cursor: `top` becomes the
        // smallest scroll that still shows the whole cursor row (capped at
        // one screen). With wrap off every height is 1 and this reduces to
        // `cursor + 1 - height`. O(viewport) per keystroke, not O(rows).
        let mut used = self.row_height(self.cursor).min(height);
        let mut top = self.cursor;
        while top > self.scroll {
            let above = self.row_height(top - 1);
            if used + above > height {
                break;
            }
            used += above;
            top -= 1;
        }
        if self.scroll < top {
            self.scroll = top;
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

/// The only place state changes (ADR-0003). Returns the side effect the
/// runtime should execute, if the message requested one.
pub fn update(state: &mut AppState, msg: Msg) -> Option<Command> {
    let pending_g = std::mem::take(&mut state.pending_g);
    // One-shot feedback: the next user interaction clears the previous
    // message. Resize and command results are not interactions.
    if !matches!(msg, Msg::CommandFinished(_) | Msg::Resize(..)) {
        state.status_message = None;
    }
    let mut command = None;

    match msg {
        Msg::StageHunk => command = state.request_hunk_apply(HunkAction::Stage),
        Msg::UnstageHunk => command = state.request_hunk_apply(HunkAction::Unstage),
        Msg::DiscardHunk => state.request_discard(),
        Msg::ConfirmInput(c) => {
            if let Some(confirm) = &mut state.confirm {
                // The input renders on the status line: control characters
                // must never reach the terminal (SECURITY.md).
                if !c.is_control() {
                    confirm.input.push(c);
                }
            }
        }
        Msg::ConfirmBackspace => {
            if let Some(confirm) = &mut state.confirm {
                confirm.input.pop();
            }
        }
        Msg::ConfirmSubmit => {
            if let Some(confirm) = state.confirm.take() {
                if confirm.input.trim().eq_ignore_ascii_case("yes") {
                    command = Some(Command::DiscardHunk {
                        backup: confirm.backup,
                        patch: confirm.patch,
                    });
                } else {
                    state.status_message = Some("discard cancelled (only `yes` confirms)".into());
                }
            }
        }
        Msg::ConfirmCancel => {
            if state.confirm.take().is_some() {
                state.status_message = Some("discard cancelled".into());
            }
        }
        // Always issued, even on an empty changeset — a reload may be
        // exactly what brings changes into view.
        Msg::Reload => command = Some(Command::Reload),
        Msg::ToggleViewed => {
            if let Some(idx) = state.current_file() {
                let keyed = state
                    .changeset
                    .files
                    .get(idx)
                    .and_then(|file| path_key(file).map(|k| (k.to_vec(), file)));
                if let Some((key, file)) = keyed {
                    if state.viewed.remove(&key).is_some() {
                        // Un-viewing reopens the file.
                        state.fold.insert(key, false);
                    } else {
                        state
                            .viewed
                            .insert(key.clone(), margin_core::file_digest(file));
                        state.fold.insert(key, true);
                    }
                    state.rebuild_rows_after_fold(Some(idx));
                    command = Some(state.save_viewed_command());
                }
            }
        }
        Msg::ZKey => state.pending_z = true,
        Msg::FoldCancel => state.pending_z = false,
        Msg::ToggleFold => {
            state.pending_z = false;
            if let Some(file) = state.current_file() {
                if let Some(key) = state.changeset.files.get(file).and_then(path_key) {
                    let key = key.to_vec();
                    let entry = state.fold.entry(key).or_insert(false);
                    *entry = !*entry;
                }
                state.rebuild_rows_after_fold(Some(file));
            }
        }
        Msg::ToggleFoldAll => {
            state.pending_z = false;
            let file = state.current_file();
            // Any expanded file means "collapse everything"; only when
            // all are folded does zA expand everything back.
            let target = state.fold.values().any(|&collapsed| !collapsed);
            for collapsed in state.fold.values_mut() {
                *collapsed = target;
            }
            state.rebuild_rows_after_fold(file);
        }
        Msg::CommandFinished(result) => state.finish_command(result),
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
        Msg::ToggleWrap => state.wrap = !state.wrap,
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
    command
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
/// Collapsed files contribute their header only — the body rows never
/// exist, so navigation and scroll math skip them by construction.
fn build_rows(changeset: &Changeset, split: bool, collapsed: &[bool]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (file, diff) in changeset.files.iter().enumerate() {
        rows.push(Row::FileHeader { file });
        if collapsed.get(file).copied().unwrap_or(false) {
            continue;
        }
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
