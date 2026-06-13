//! Frame snapshot tests (ADR-0010 layer 2): render a known changeset at
//! fixed terminal sizes via ratatui's TestBackend and snapshot the text.
//! Keybinding flows are Msg sequences followed by a snapshot — the snapshot
//! diff in a PR *is* the UI review.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use insta::assert_snapshot;
use margin_core::parse_unified;
use margin_tui::{render_view, update, AppState, Msg};
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
