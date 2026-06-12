//! Lazy syntax highlighting and intra-line emphasis with a per-frame work
//! budget (ADR-0006).
//!
//! ## Why this shape
//!
//! Highlighting is the expensive part of rendering and must never block the
//! input loop. The cache is memoized per `(file, hunk)`: a hunk's lines are
//! highlighted in order (syntect's parser is stateful), at most
//! `FRAME_BUDGET` lines per frame across the whole app. Rows whose
//! highlighting hasn't been computed yet render plain and fill in on
//! subsequent frames — the runtime polls with a short timeout while work is
//! pending, so the fill-in happens within milliseconds without an input
//! event, and idles at zero CPU otherwise.
//!
//! Interior mutability keeps `view()` pure in the way that matters:
//! rendering the same state twice yields the same frame; the cache only
//! memoizes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;

use margin_core::{intraline_ranges, paired_changes, Hunk};
use ratatui::style::{Color, Modifier, Style};
use syntect::highlighting::{
    FontStyle, HighlightIterator, HighlightState, Highlighter as SynHighlighter, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

use crate::view::printable;

/// Maximum lines highlighted per frame, app-wide. ~400 lines costs single-
/// digit milliseconds in release builds; a 250k-line hunk fills in across
/// frames instead of freezing the first one.
const FRAME_BUDGET: usize = 400;

/// Hunks beyond this many lines skip syntax highlighting entirely (intra-
/// line emphasis still applies): generated/lockfile blobs gain nothing
/// from coloring and would churn the cache.
const MAX_HUNK_LINES: usize = 50_000;

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_nonewlines)
}

fn syntect_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let mut themes = ThemeSet::load_defaults().themes;
        // Until issue #6 brings configurable themes, one tasteful default.
        themes
            .remove("base16-ocean.dark")
            .or_else(|| themes.into_values().next())
            .unwrap_or_default()
    })
}

/// What the view needs to render one diff line.
#[derive(Debug, Clone, Default)]
pub struct LineRender {
    /// Syntax-colored pieces covering the printable content, in order.
    /// `None` while not yet computed (render plain) or not applicable.
    pub syntax: Option<Vec<(Style, String)>>,
    /// Byte ranges (into the printable content) to emphasize.
    pub emphasis: Vec<Range<usize>>,
}

pub struct HighlightCache {
    state: RefCell<CacheState>,
}

#[derive(Default)]
struct CacheState {
    hunks: HashMap<(usize, usize), HunkCache>,
    budget: usize,
    pending: bool,
}

struct HunkCache {
    /// Syntax name resolved from the file extension; `None` = plain.
    syntax: Option<String>,
    parse: Option<(ParseState, HighlightState)>,
    next_line: usize,
    lines: Vec<Option<Vec<(Style, String)>>>,
    /// deletion-index <-> addition-index partners for emphasis.
    partners: HashMap<usize, usize>,
    emphasis: HashMap<usize, Vec<Range<usize>>>,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self {
            state: RefCell::new(CacheState::default()),
        }
    }
}

impl HighlightCache {
    /// Reset the per-frame work budget. Called once at the top of `view`.
    pub fn begin_frame(&self) {
        let mut state = self.state.borrow_mut();
        state.budget = FRAME_BUDGET;
        state.pending = false;
    }

    /// True when budget ran out before all requested lines were computed —
    /// the runtime uses this to schedule a fill-in redraw.
    pub fn has_pending(&self) -> bool {
        self.state.borrow().pending
    }

    /// Rendering data for `hunk.lines[line]` of file `file_idx` (`path` is
    /// the display path used for syntax detection).
    pub fn line_render(
        &self,
        file_idx: usize,
        hunk_idx: usize,
        path: &str,
        hunk: &Hunk,
        line: usize,
    ) -> LineRender {
        let mut state = self.state.borrow_mut();
        // Split borrows: the entry, the budget, and the pending flag are
        // independent fields of the cache state.
        let CacheState {
            hunks,
            budget,
            pending,
        } = &mut *state;
        let entry = hunks
            .entry((file_idx, hunk_idx))
            .or_insert_with(|| HunkCache::new(path, hunk));

        // Emphasis: computed per pair on first request, both sides at once.
        if !entry.emphasis.contains_key(&line) {
            if let Some(&partner) = entry.partners.get(&line) {
                let (a, b) = (usize::min(line, partner), usize::max(line, partner));
                if let (Some(old), Some(new)) = (hunk.lines.get(a), hunk.lines.get(b)) {
                    let old_text = printable(&old.content);
                    let new_text = printable(&new.content);
                    let (old_ranges, new_ranges) = intraline_ranges(&old_text, &new_text);
                    entry.emphasis.insert(a, old_ranges);
                    entry.emphasis.insert(b, new_ranges);
                }
            } else {
                entry.emphasis.insert(line, Vec::new());
            }
        }

        // Syntax: advance the stateful parser toward this line within budget.
        if entry.syntax.is_some() && entry.advance_to(line, hunk, budget) {
            *pending = true;
        }

        LineRender {
            syntax: entry.lines.get(line).cloned().flatten(),
            emphasis: entry.emphasis.get(&line).cloned().unwrap_or_default(),
        }
    }
}

impl HunkCache {
    fn new(path: &str, hunk: &Hunk) -> Self {
        let syntax = if hunk.lines.len() > MAX_HUNK_LINES {
            None
        } else {
            path.rsplit('.')
                .next()
                .filter(|ext| !ext.is_empty() && !ext.contains('/'))
                .and_then(|ext| syntax_set().find_syntax_by_extension(ext))
                .map(|s| s.name.clone())
        };
        Self {
            syntax,
            parse: None,
            next_line: 0,
            lines: vec![None; hunk.lines.len()],
            partners: paired_changes(hunk)
                .into_iter()
                .flat_map(|(d, a)| [(d, a), (a, d)])
                .collect(),
            emphasis: HashMap::new(),
        }
    }

    /// Highlight lines `next_line..=target` while budget lasts. Returns true
    /// when the budget ran out first.
    fn advance_to(&mut self, target: usize, hunk: &Hunk, budget: &mut usize) -> bool {
        if self.next_line > target {
            return false;
        }
        let Some(name) = self.syntax.clone() else {
            return false;
        };
        let Some(syntax) = syntax_set().find_syntax_by_name(&name) else {
            self.syntax = None;
            return false;
        };
        let highlighter = SynHighlighter::new(syntect_theme());
        if self.parse.is_none() {
            self.parse = Some((
                ParseState::new(syntax),
                HighlightState::new(&highlighter, ScopeStack::new()),
            ));
        }

        while self.next_line <= target {
            if *budget == 0 {
                return true;
            }
            let Some(line) = hunk.lines.get(self.next_line) else {
                return false;
            };
            let content = printable(&line.content);
            let Some((parse_state, hl_state)) = self.parse.as_mut() else {
                return false;
            };
            let spans = match parse_state.parse_line(&content, syntax_set()) {
                Ok(ops) => HighlightIterator::new(hl_state, &ops, &content, &highlighter)
                    .map(|(style, piece)| (convert_style(style), piece.to_string()))
                    .collect::<Vec<_>>(),
                Err(_) => {
                    // Parser bailed (pathological input): degrade this hunk
                    // to plain rendering rather than erroring.
                    self.syntax = None;
                    return false;
                }
            };
            if let Some(slot) = self.lines.get_mut(self.next_line) {
                *slot = Some(spans);
            }
            self.next_line += 1;
            *budget -= 1;
        }
        false
    }
}

/// Map a syntect style to a ratatui style: foreground + font modifiers.
/// Backgrounds stay ours (addition/deletion tints, cursor bar).
fn convert_style(style: syntect::highlighting::Style) -> Style {
    let fg = style.foreground;
    let mut out = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    if style.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test scaffolding may panic
mod tests {
    use super::*;
    use margin_core::parse_unified;

    fn rust_hunk() -> (margin_core::Changeset, Hunk) {
        let patch =
            b"--- a/x.rs\n+++ b/x.rs\n@@ -1,3 +1,3 @@\n fn ctx() {}\n-let a = 1;\n+let a = 2;\n";
        let cs = parse_unified(patch).changeset;
        let hunk = cs.files[0].hunks[0].clone();
        (cs, hunk)
    }

    #[test]
    fn rust_lines_get_syntax_spans_and_emphasis() {
        let (_cs, hunk) = rust_hunk();
        let cache = HighlightCache::default();
        cache.begin_frame();
        let render = cache.line_render(0, 0, "x.rs", &hunk, 1);
        let spans = render.syntax.expect("rust line should highlight");
        let total: usize = spans.iter().map(|(_, s)| s.len()).sum();
        assert_eq!(total, "let a = 1;".len(), "spans cover the content");
        // The deletion pairs with the addition; only the literal differs.
        assert_eq!(render.emphasis, vec![8..10], "token is 1; not just 1");
    }

    #[test]
    fn unknown_extension_renders_plain_but_still_emphasizes() {
        let (_cs, hunk) = rust_hunk();
        let cache = HighlightCache::default();
        cache.begin_frame();
        let render = cache.line_render(0, 0, "data.unknownext", &hunk, 2);
        assert!(render.syntax.is_none());
        assert_eq!(render.emphasis, vec![8..10], "token is 1; not just 1");
    }

    #[test]
    fn budget_exhaustion_reports_pending_and_resumes() {
        let mut lines = String::new();
        for i in 0..1000 {
            lines.push_str(&format!("+let v{i} = {i};\n"));
        }
        let patch = format!("--- a/x.rs\n+++ b/x.rs\n@@ -0,0 +1,1000 @@\n{lines}");
        let cs = parse_unified(patch.as_bytes()).changeset;
        let hunk = &cs.files[0].hunks[0];

        let cache = HighlightCache::default();
        cache.begin_frame();
        let last = cache.line_render(0, 0, "x.rs", hunk, 999);
        assert!(
            last.syntax.is_none(),
            "budget cannot reach line 999 in one frame"
        );
        assert!(cache.has_pending());

        // Two more frames of budget catch up.
        cache.begin_frame();
        let _ = cache.line_render(0, 0, "x.rs", hunk, 999);
        cache.begin_frame();
        let done = cache.line_render(0, 0, "x.rs", hunk, 999);
        assert!(done.syntax.is_some(), "fill-in completes across frames");
        assert!(!cache.has_pending());
    }
}
