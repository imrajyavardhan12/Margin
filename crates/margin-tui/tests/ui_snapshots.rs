//! Frame snapshot tests (ADR-0010 layer 2): render a known changeset at
//! fixed terminal sizes via ratatui's TestBackend and snapshot the text.
//! Keybinding flows are Msg sequences followed by a snapshot — the snapshot
//! diff in a PR *is* the UI review.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use insta::assert_snapshot;
use margin_core::parse_unified;
use margin_tui::app::Row;
use margin_tui::{
    render_view, update, AppState, Command, CommandResult, HunkAction, Msg, StagedFiles,
};
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
    let Command::ApplyHunk { action, patch } = command else {
        panic!("expected an apply command");
    };
    assert_eq!(action, HunkAction::Stage);
    assert!(patch.starts_with(b"diff --git"));
    assert!(parse_unified(&patch).warnings.is_empty());

    let reloaded = state.changeset.clone();
    // The runtime hands back a fresh index-vs-HEAD summary. Staging one hunk
    // of src/app.rs puts only that file in the index, so only it is marked.
    let staged = StagedFiles::from_staged_changeset(&margin_core::Changeset {
        files: vec![reloaded.files[0].clone()],
    });
    assert!(
        staged.is_staged(&state.changeset.files[0]),
        "the reloaded summary should mark the staged file"
    );
    assert_eq!(
        update(
            &mut state,
            Msg::CommandFinished(CommandResult::Applied {
                action,
                changeset: reloaded,
                staged: Some(staged),
            }),
        ),
        None
    );
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("hunk staged"), "{frame}");
    // The staged dot rides in the sidebar's reserved status column.
    assert!(
        frame.contains('\u{25cf}'),
        "staged indicator must show: {frame}"
    );
    assert_snapshot!(frame);

    update(&mut state, Msg::CursorDown);
    assert!(!render(&mut state, 80, 24).contains("hunk staged"));
}

/// The sidebar marks files with staged content and leaves the rest alone.
#[test]
fn sidebar_marks_staged_files() {
    let mut state = sample_state();
    // Pretend docs/NOTES.md has been staged (index vs HEAD). The `diff --git`
    // header puts the parser in git-prefix mode so the path strips to
    // `docs/NOTES.md`, matching the worktree file it annotates.
    let staged_patch = b"diff --git a/docs/NOTES.md b/docs/NOTES.md\n\
        --- a/docs/NOTES.md\n+++ b/docs/NOTES.md\n@@ -1,1 +1,1 @@\n-old\n+new\n";
    state.staged = Some(StagedFiles::from_staged_changeset(
        &parse_unified(staged_patch).changeset,
    ));

    let frame = render(&mut state, 80, 24);
    assert!(
        frame.contains('\u{25cf}'),
        "a staged file must show the indicator: {frame}"
    );
    // Exactly one file is staged, so exactly one dot appears.
    assert_eq!(
        frame.matches('\u{25cf}').count(),
        1,
        "only the staged file is marked: {frame}"
    );
    assert_snapshot!(frame);
}

#[test]
fn stage_refuses_off_hunk_and_reports_stale() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    // Cursor starts on the file header: no hunk to stage.
    assert_eq!(update(&mut state, Msg::StageHunk), None);
    assert!(render(&mut state, 80, 24).contains("no hunk under the cursor"));

    // The honest diagnosis differs by direction (a failed stage is usually
    // already staged; a failed unstage usually wasn't staged).
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Stale(HunkAction::Stage)),
    );
    assert!(render(&mut state, 80, 24).contains("already staged"));
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Stale(HunkAction::Unstage)),
    );
    assert!(render(&mut state, 80, 24).contains("isn't staged"));

    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Unsupported("staging needs a git review")),
    );
    assert!(render(&mut state, 80, 24).contains("staging needs a git review"));
}

/// With an authoritative staged summary, unstaging a file with no index
/// content refuses purely (no command); without one (`--staged` reviews),
/// the request must still go through — the summary being absent is not
/// the same as nothing being staged.
#[test]
fn unstage_precheck_uses_the_staged_summary() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::NextHunk);
    update(&mut state, Msg::CursorDown);

    state.staged = Some(StagedFiles::default());
    assert_eq!(
        update(&mut state, Msg::UnstageHunk),
        None,
        "nothing staged: refuse without issuing a command"
    );
    assert!(render(&mut state, 80, 24).contains("nothing staged in this file"));

    state.staged = None;
    let command = update(&mut state, Msg::UnstageHunk);
    assert!(
        matches!(
            command,
            Some(Command::ApplyHunk {
                action: HunkAction::Unstage,
                ..
            })
        ),
        "no summary: the request must reach the index, got {command:?}"
    );
}

/// `x` opens the typed confirmation; only `yes` + Enter yields the
/// discard command, carrying the reversed patch and the forward backup
/// (ADR-0014). The result message points at `margin undo`.
#[test]
fn discard_flow_requires_typed_yes() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::NextHunk);
    update(&mut state, Msg::CursorDown);

    assert_eq!(update(&mut state, Msg::DiscardHunk), None, "x never writes");
    assert_eq!(state.input_mode(), margin_tui::app::InputMode::Confirm);
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("type yes"), "{frame}");
    assert!(
        frame.contains("src/app.rs"),
        "prompt names the file: {frame}"
    );
    assert_snapshot!(frame);

    for c in "yes".chars() {
        assert_eq!(update(&mut state, Msg::ConfirmInput(c)), None);
    }
    let command = update(&mut state, Msg::ConfirmSubmit).expect("yes + Enter issues the command");
    let Command::DiscardHunk { backup, patch } = command else {
        panic!("expected a discard command");
    };
    assert!(state.confirm.is_none(), "prompt closes on submit");
    // Backup is the forward hunk (what was on screen); patch is its
    // reverse. Both must reparse cleanly.
    assert!(backup.starts_with(b"diff --git"));
    assert!(parse_unified(&backup).warnings.is_empty());
    assert!(parse_unified(&patch).warnings.is_empty());
    assert_ne!(backup, patch);

    let reloaded = state.changeset.clone();
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Discarded {
            changeset: reloaded,
            staged: None,
            backed_up: true,
        }),
    );
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("margin undo"), "{frame}");
}

/// Anything except `yes` cancels; Esc cancels; a hunk that cannot be
/// rendered refuses before the prompt ever opens.
#[test]
fn discard_cancels_and_refuses_safely() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));

    // On the file header there is no hunk: no prompt, message instead.
    update(&mut state, Msg::DiscardHunk);
    assert!(state.confirm.is_none());
    assert!(render(&mut state, 80, 24).contains("no hunk under the cursor"));

    update(&mut state, Msg::NextHunk);
    update(&mut state, Msg::DiscardHunk);
    for c in "no".chars() {
        update(&mut state, Msg::ConfirmInput(c));
    }
    assert_eq!(update(&mut state, Msg::ConfirmSubmit), None);
    assert!(state.confirm.is_none());
    assert!(render(&mut state, 80, 24).contains("only `yes` confirms"));

    update(&mut state, Msg::DiscardHunk);
    assert_eq!(update(&mut state, Msg::ConfirmCancel), None);
    assert!(render(&mut state, 80, 24).contains("discard cancelled"));

    // A path that would need git quoting refuses without prompting.
    let hostile = "diff --git \"a/e\\033vil.rs\" \"b/e\\033vil.rs\"\n\
                   --- \"a/e\\033vil.rs\"\n+++ \"b/e\\033vil.rs\"\n\
                   @@ -1,1 +1,1 @@\n-a\n+b\n";
    let mut state = AppState::new(parse_unified(hostile.as_bytes()).changeset);
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::NextHunk);
    update(&mut state, Msg::DiscardHunk);
    assert!(state.confirm.is_none(), "unsafe path must not prompt");
    assert!(render(&mut state, 80, 24).contains("needs git quoting"));

    // Without a trash entry the success message says so.
    let mut state = sample_state();
    let reloaded = state.changeset.clone();
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Discarded {
            changeset: reloaded,
            staged: None,
            backed_up: false,
        }),
    );
    assert!(render(&mut state, 80, 24).contains("backup disabled"));
}

/// A changeset where a lockfile dwarfs the real change (issue #21).
const WITH_LOCKFILE: &str = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,2 +1,2 @@
 fn keep() {}
-fn old() {}
+fn renamed() {}
diff --git a/Cargo.lock b/Cargo.lock
index 3333333..4444444 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,4 +1,4 @@
 [[package]]
 name = \"serde\"
-version = \"1.0.1\"
+version = \"1.0.2\"
 checksum = \"abc\"
";

fn lockfile_state() -> AppState {
    let outcome = parse_unified(WITH_LOCKFILE.as_bytes());
    assert!(outcome.warnings.is_empty(), "fixture must parse cleanly");
    AppState::new(outcome.changeset)
}

/// Lockfiles fold on load: header only (with the fold marker and counts),
/// body rows never built — navigation skips them by construction.
#[test]
fn lockfile_auto_collapses_to_header_only() {
    let mut state = lockfile_state();
    assert_eq!(
        state.rows.iter().filter(|r| r.file() == 1).count(),
        1,
        "collapsed file contributes exactly its header row"
    );
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains('\u{25b8}'), "fold marker shows: {frame}");
    assert!(
        frame.contains("Cargo.lock  +1 -1"),
        "counts stay visible: {frame}"
    );
    assert!(!frame.contains("serde"), "body stays hidden: {frame}");
    assert_snapshot!(frame);

    // J from the first file's hunk jumps clean over the folded body.
    update(&mut state, Msg::Bottom);
    assert!(
        matches!(state.rows[state.cursor], Row::FileHeader { file: 1 }),
        "the folded file ends at its header"
    );
}

/// `za` on the folded header expands it; `za` again folds it back.
#[test]
fn za_toggles_the_cursor_file() {
    let mut state = lockfile_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::Bottom); // the collapsed lockfile header

    update(&mut state, Msg::ZKey);
    assert_eq!(state.input_mode(), margin_tui::app::InputMode::Fold);
    update(&mut state, Msg::ToggleFold);
    assert_eq!(state.input_mode(), margin_tui::app::InputMode::Normal);
    let frame = render(&mut state, 80, 24);
    assert!(frame.contains("serde"), "expanded body renders: {frame}");
    assert!(
        matches!(state.rows[state.cursor], Row::FileHeader { file: 1 }),
        "cursor stays on the toggled file's header"
    );

    update(&mut state, Msg::ZKey);
    update(&mut state, Msg::ToggleFold);
    assert!(!render(&mut state, 80, 24).contains("serde"));

    // Any non-fold key breaks the chord without side effects.
    update(&mut state, Msg::ZKey);
    update(&mut state, Msg::FoldCancel);
    assert_eq!(state.input_mode(), margin_tui::app::InputMode::Normal);
}

/// `zA` folds everything while anything is expanded, then unfolds all.
#[test]
fn za_all_toggles_everything() {
    let mut state = lockfile_state();
    update(&mut state, Msg::Resize(80, 24));

    update(&mut state, Msg::ToggleFoldAll);
    assert_eq!(
        state.rows.len(),
        2,
        "all collapsed: two header rows only, got {:?}",
        state.rows
    );

    update(&mut state, Msg::ToggleFoldAll);
    assert!(
        state.rows.len() > 2,
        "everything folded, so zA expands all — even the lockfile"
    );
    assert!(render(&mut state, 80, 24).contains("serde"));
}

/// The user's fold choice survives a reload (watch mode reloads often).
#[test]
fn fold_overrides_survive_reload() {
    let mut state = lockfile_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::Bottom);
    update(&mut state, Msg::ZKey);
    update(&mut state, Msg::ToggleFold); // expand the lockfile

    let reloaded = state.changeset.clone();
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Reloaded {
            changeset: reloaded,
            staged: None,
        }),
    );
    assert!(
        render(&mut state, 80, 24).contains("serde"),
        "an explicitly expanded file must not re-fold on reload"
    );
}

/// Config globs extend the built-in heuristics.
#[test]
fn collapse_globs_fold_matching_files() {
    let mut state = sample_state(); // has docs/NOTES.md
    state.set_collapse_globs(vec!["*.md".into()]);
    let frame = render(&mut state, 80, 24);
    assert!(
        !frame.contains("remember the milk"),
        "globbed file folds: {frame}"
    );
    assert!(frame.contains("NOTES.md"), "its header stays: {frame}");
}

/// A reload while the picker is open must refilter it: `filtered` holds
/// file *indices*, and stale indices confirm-jump to whatever file now
/// occupies the old position (post-M2 review finding).
#[test]
fn reload_refilters_an_open_picker() {
    let two = b"--- a/src/parser.rs\n+++ b/src/parser.rs\n@@ -1,1 +1,1 @@\n-a\n+b\n\
--- a/docs/notes.md\n+++ b/docs/notes.md\n@@ -1,1 +1,1 @@\n-c\n+d\n";
    let mut state = AppState::new(parse_unified(two).changeset);
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::PickerStart);
    for c in "notes".chars() {
        update(&mut state, Msg::PickerInput(c));
    }
    assert_eq!(
        state.picker.as_ref().map(|p| p.filtered.clone()),
        Some(vec![1])
    );

    // The world moves under the open picker: a new file lands first,
    // shifting notes.md from index 1 to index 2.
    let three = b"--- a/aaa.txt\n+++ b/aaa.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n\
--- a/src/parser.rs\n+++ b/src/parser.rs\n@@ -1,1 +1,1 @@\n-a\n+b\n\
--- a/docs/notes.md\n+++ b/docs/notes.md\n@@ -1,1 +1,1 @@\n-c\n+d\n";
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Reloaded {
            changeset: parse_unified(three).changeset,
            staged: None,
        }),
    );
    assert_eq!(
        state.picker.as_ref().map(|p| p.filtered.clone()),
        Some(vec![2]),
        "filtered indices must track the reloaded changeset"
    );
    update(&mut state, Msg::PickerConfirm);
    assert!(
        matches!(state.rows[state.cursor], Row::FileHeader { file: 2 }),
        "confirm lands on notes.md, not whatever holds the stale index"
    );
}

/// Watch mode announces itself in the status bar like [split]/[wrap].
#[test]
fn watch_badge_shows_when_watching() {
    let mut state = sample_state();
    assert!(!render(&mut state, 80, 24).contains("[watch]"));
    state.watching = true;
    assert!(render(&mut state, 80, 24).contains("[watch]"));
}

/// `r` requests a reload; absorbing the result keeps the cursor's place
/// and reports in the status bar.
#[test]
fn reload_requests_and_absorbs() {
    let mut state = sample_state();
    update(&mut state, Msg::Resize(80, 24));
    update(&mut state, Msg::NextHunk);
    let anchor = state.rows[state.cursor];

    assert_eq!(update(&mut state, Msg::Reload), Some(Command::Reload));
    let reloaded = state.changeset.clone();
    update(
        &mut state,
        Msg::CommandFinished(CommandResult::Reloaded {
            changeset: reloaded,
            staged: None,
        }),
    );
    assert_eq!(state.rows[state.cursor], anchor, "cursor keeps its place");
    assert!(render(&mut state, 80, 24).contains("reloaded"));

    // An empty review still offers reload — it may bring changes into view.
    let mut empty = AppState::new(margin_core::Changeset::default());
    assert_eq!(update(&mut empty, Msg::Reload), Some(Command::Reload));
}
