//! Frame snapshot tests (ADR-0010 layer 2): render a known changeset at
//! fixed terminal sizes via ratatui's TestBackend and snapshot the text.
//! Keybinding flows are Msg sequences followed by a snapshot — the snapshot
//! diff in a PR *is* the UI review.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use insta::assert_snapshot;
use margin_core::parse_unified;
use margin_tui::{render_view, update, AppState, Command, CommandResult, HunkAction, Msg};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

/// A changeset with one of everything: a modified file with two hunks and a
/// heading, an added file, a binary file, and a pure rename.
const SAMPLE: &str = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,5 +1,6 @@ fn setup()
 use std::env;

-fn setup() {
-    init(\"defaults\");
+fn setup() {
+    let profile = env::var(\"PROFILE\");
+    init(profile.as_deref().unwrap_or(\"defaults\"));
 }
@@ -20,3 +21,4 @@
 fn teardown() {
     cleanup();
 }
+// reviewed
diff --git a/docs/NOTES.md b/docs/NOTES.md
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/docs/NOTES.md
@@ -0,0 +1,2 @@
+# Notes
+remember the milk
diff --git a/assets/logo.png b/assets/logo.png
index 4444444..5555555 100644
Binary files a/assets/logo.png and b/assets/logo.png differ
diff --git a/old/path.txt b/new/path.txt
similarity index 100%
rename from old/path.txt
rename to new/path.txt
";

fn sample_state() -> AppState {
    let outcome = parse_unified(SAMPLE.as_bytes());
    assert!(outcome.warnings.is_empty(), "sample must parse cleanly");
    AppState::new(outcome.changeset)
}

fn render(state: &mut AppState, width: u16, height: u16) -> String {
    update(state, Msg::Resize(width, height));
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_view(state, frame)).unwrap();
    buffer_text(terminal.backend().buffer())
}

fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area();
    (0..area.height)
        .map(|y| {
            let line: String = (0..area.width).map(|x| buffer[(x, y)].symbol()).collect();
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn standard_80x24() {
    let mut state = sample_state();
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn wide_200x50() {
    let mut state = sample_state();
    assert_snapshot!(render(&mut state, 200, 50));
}

#[test]
fn narrow_40x20_hides_sidebar() {
    let mut state = sample_state();
    let frame = render(&mut state, 40, 20);
    assert!(!frame.contains("FILES"), "sidebar must yield at 40 cols");
    assert_snapshot!(frame);
}

#[test]
fn navigation_flow_lands_on_second_file() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    for msg in [Msg::NextHunk, Msg::NextHunk, Msg::NextFile] {
        update(&mut state, msg);
    }
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn cursor_scrolls_into_view_at_bottom() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 12));
    update(&mut state, Msg::Bottom);
    assert_snapshot!(render(&mut state, 80, 12));
}

#[test]
fn help_overlay() {
    let mut state = sample_state();
    update(&mut state, Msg::ToggleHelp);
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn sidebar_toggled_off() {
    let mut state = sample_state();
    update(&mut state, Msg::ToggleSidebar);
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn split_auto_at_160_cols() {
    let mut state = sample_state();
    let frame = render(&mut state, 160, 30);
    assert!(frame.contains("[split]"), "auto layout must pick split");
    assert_snapshot!(frame);
}

#[test]
fn forced_split_at_80_cols_via_v() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::ToggleLayout);
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn forced_split_at_40x10_does_not_panic() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(40, 10));
    update(&mut state, Msg::ToggleLayout);
    let frame = render(&mut state, 40, 10);
    assert!(!frame.is_empty());
}

#[test]
fn empty_changeset() {
    let mut state = AppState::new(margin_core::Changeset::default());
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn search_bar_while_typing() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::SearchStart);
    for c in "setup".chars() {
        update(&mut state, Msg::SearchInput(c));
    }
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("/setup"), "{frame}");
    assert!(frame.contains("matching rows"), "{frame}");
    assert_snapshot!(frame);
}

#[test]
fn confirmed_search_shows_badge_and_jumps() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::SearchStart);
    for c in "setup".chars() {
        update(&mut state, Msg::SearchInput(c));
    }
    update(&mut state, Msg::SearchConfirm);
    update(&mut state, Msg::NextMatch);
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("/setup"), "badge visible: {frame}");
    assert_snapshot!(frame);
}

#[test]
fn picker_overlay_filters() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::PickerStart);
    for c in "notes".chars() {
        update(&mut state, Msg::PickerInput(c));
    }
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("jump to file (1/4)"), "{frame}");
    assert!(frame.contains("NOTES.md"), "{frame}");
    assert_snapshot!(frame);
}

/// One file whose lines blow past 80 columns, including a full-width CJK
/// line — the wrap (`w`) fixtures.
const LONG_LINES: &str = "\
diff --git a/src/long.rs b/src/long.rs
index aaaaaaa..bbbbbbb 100644
--- a/src/long.rs
+++ b/src/long.rs
@@ -1,2 +1,3 @@ fn long()
 short context line
-let removed = compose(\"a deliberately long deleted line that sails far past eighty columns so the unified pane must wrap it onto several visual rows\");
+let added = compose(\"a deliberately long added line that also sails far past eighty columns and keeps going for a while longer so wrapping gets more than one continuation row\");
+// \u{5bbd}\u{5b57}\u{7b26}\u{6d4b}\u{8bd5}\u{ff1a}\u{8fd9}\u{4e00}\u{884c}\u{5305}\u{542b}\u{5168}\u{89d2}\u{5b57}\u{7b26}\u{ff0c}\u{7528}\u{6765}\u{9a8c}\u{8bc1}\u{6309}\u{663e}\u{793a}\u{5bbd}\u{5ea6}\u{6362}\u{884c}\u{7684}\u{6b63}\u{786e}\u{6027}\u{ff0c}\u{7edd}\u{4e0d}\u{80fd}\u{628a}\u{4e00}\u{4e2a}\u{5b57}\u{7b26}\u{5288}\u{6210}\u{4e24}\u{534a}
";

fn long_state() -> AppState {
    let outcome = parse_unified(LONG_LINES.as_bytes());
    assert!(outcome.warnings.is_empty(), "fixture must parse cleanly");
    AppState::new(outcome.changeset)
}

#[test]
fn wrapped_unified_80_cols() {
    let mut state = long_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::ToggleWrap);
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("[wrap]"), "status must show the wrap badge");
    assert_snapshot!(frame);
}

#[test]
fn wrapped_split_80_cols() {
    let mut state = long_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::ToggleLayout);
    update(&mut state, Msg::ToggleWrap);
    assert_snapshot!(render(&mut state, 80, 24));
}

#[test]
fn wrap_at_40x10_does_not_panic() {
    let mut state = long_state();
    update(&mut state, Msg::Resize(40, 10));
    update(&mut state, Msg::ToggleWrap);
    update(&mut state, Msg::Bottom);
    assert!(!render(&mut state, 40, 10).is_empty());
    // Same terminal, forced split: halves get tiny, still no panic.
    update(&mut state, Msg::ToggleLayout);
    update(&mut state, Msg::GKey);
    update(&mut state, Msg::GKey);
    update(&mut state, Msg::Bottom);
    assert!(!render(&mut state, 40, 10).is_empty());
}

/// The invariant behind height-aware scrolling: the cursor row's full
/// wrapped height (capped at one screen) fits between `scroll` and the
/// bottom of the viewport.
#[test]
fn wrapped_cursor_fully_visible_at_bottom() {
    let mut state = long_state();
    update(&mut state, Msg::Resize(80, 8));
    update(&mut state, Msg::ToggleWrap);
    update(&mut state, Msg::Bottom);
    let height = state.content_height();
    let visual: usize = (state.scroll..=state.cursor)
        .map(|i| state.row_height(i))
        .sum();
    assert!(state.cursor >= state.scroll);
    assert!(
        visual <= height,
        "cursor row overflows the viewport: {visual} > {height}"
    );
    assert_snapshot!(render(&mut state, 80, 8));
}

/// A crafted filename (the parser decodes `\033` octal escapes in quoted
/// paths) must never put a control character into the rendered frame —
/// not via the sidebar, the file header, or the status bar (SECURITY.md).
#[test]
fn hostile_path_never_reaches_the_frame_raw() {
    let patch = "diff --git \"a/\\033]0;pwned\\007.rs\" \"b/\\033]0;pwned\\007.rs\"\n\
                 index 1111111..2222222 100644\n\
                 --- \"a/\\033]0;pwned\\007.rs\"\n\
                 +++ \"b/\\033]0;pwned\\007.rs\"\n\
                 @@ -1,1 +1,1 @@\n\
                 -safe\n\
                 +also safe\n";
    let mut state = AppState::new(parse_unified(patch.as_bytes()).changeset);
    let frame = render(&mut state, 80, 24);
    assert!(
        !frame.chars().any(|c| c.is_control() && c != '\n'),
        "control characters leaked into the frame"
    );
    assert!(
        frame.contains("pwned"),
        "the path itself should still render"
    );
}

/// `s` on a diff line yields the staging command carrying a reparsable
/// single-hunk patch; the result message shows and the next key clears it.
#[test]
fn stage_request_result_and_clear_flow() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::NextHunk);
    update(&mut state, Msg::CursorDown);
    let command = update(&mut state, Msg::StageHunk).expect("line row must yield a command");
    let Command::ApplyHunk { action, patch } = command;
    assert_eq!(action, HunkAction::Stage);
    assert!(patch.starts_with(b"diff --git"));
    assert!(parse_unified(&patch).warnings.is_empty());

    let reloaded = state.changeset.clone();
    assert_eq!(
        update(
            &mut state,
            Msg::CommandFinished(CommandResult::Applied {
                action,
                changeset: reloaded,
            }),
        ),
        None
    );
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("hunk staged"), "{frame}");
    assert_snapshot!(frame);

    update(&mut state, Msg::CursorDown);
    assert!(!render(&mut state, 80, 24).contains("hunk staged"));
}

#[test]
fn stage_refuses_off_hunk_and_reports_stale() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    // Cursor starts on the file header: no hunk to stage.
    assert_eq!(update(&mut state, Msg::StageHunk), None);
    assert!(render(&mut state, 80, 24).contains("no hunk under the cursor"));

    update(&mut state, Msg::CommandFinished(CommandResult::Stale));
    assert!(render(&mut state, 80, 24).contains("no longer applies"));

    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Unsupported("staging needs a git review")),
    );
    assert!(render(&mut state, 80, 24).contains("staging needs a git review"));
}
