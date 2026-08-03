//! Capability-aware Review Session orchestration (ADR-0019).
//!
//! A session pairs exactly one Changeset input with the effects valid for that
//! review mode. Constructors encode the valid combinations; the TUI continues
//! to request effects as data through `CommandExecutor`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use margin_core::{Changeset, FileStatus};
use margin_tui::theme::Theme;
use margin_tui::{AppState, Command, CommandExecutor, CommandResult};
use margin_vcs::{
    apply_patch_to_index, apply_patch_to_worktree, write_trash, DiffSource, StageError,
};

use crate::config::Config;
use crate::{notes, viewed};

/// Presentation and behavior settings shared by every Review Session.
pub(crate) struct ReviewOptions {
    config: Config,
    theme: Theme,
    json: bool,
    notes: bool,
}

impl ReviewOptions {
    pub(crate) fn new(config: Config, theme: Theme, json: bool, notes: bool) -> Self {
        Self {
            config,
            theme,
            json,
            notes,
        }
    }

    pub(crate) fn json_output(&self) -> bool {
        self.json
    }

    pub(crate) fn include_untracked(&self) -> bool {
        self.config.include_untracked
    }

    pub(crate) fn discard_backups(&self) -> bool {
        self.config.discard_trash
    }
}

/// One complete review input and the only effects it supports.
///
/// Snapshot reviews (patch/pager input) cannot accidentally acquire live-source
/// effects. Reloadable sources carry one explicit mode, so combinations such as
/// “discardable staged review” or “watched static range” cannot be constructed.
pub(crate) struct ReviewSession<'a> {
    kind: ReviewKind<'a>,
}

enum ReviewKind<'a> {
    Snapshot(Changeset),
    Reloadable {
        source: &'a dyn DiffSource,
        mode: ReloadableMode,
    },
}

enum ReloadableMode {
    ReadOnly,
    Staged {
        repo: PathBuf,
        watch: bool,
    },
    Worktree {
        repo: PathBuf,
        watch: bool,
        backup_discards: bool,
    },
}

impl<'a> ReviewSession<'a> {
    pub(crate) fn snapshot(changeset: Changeset) -> Self {
        Self {
            kind: ReviewKind::Snapshot(changeset),
        }
    }

    pub(crate) fn read_only(source: &'a dyn DiffSource) -> Self {
        Self {
            kind: ReviewKind::Reloadable {
                source,
                mode: ReloadableMode::ReadOnly,
            },
        }
    }

    pub(crate) fn staged(source: &'a dyn DiffSource, repo: PathBuf, watch: bool) -> Self {
        Self {
            kind: ReviewKind::Reloadable {
                source,
                mode: ReloadableMode::Staged { repo, watch },
            },
        }
    }

    pub(crate) fn worktree(
        source: &'a dyn DiffSource,
        repo: PathBuf,
        watch: bool,
        backup_discards: bool,
    ) -> Self {
        Self {
            kind: ReviewKind::Reloadable {
                source,
                mode: ReloadableMode::Worktree {
                    repo,
                    watch,
                    backup_discards,
                },
            },
        }
    }

    pub(crate) fn run(self, options: &ReviewOptions) -> ExitCode {
        match self.kind {
            ReviewKind::Snapshot(changeset) => {
                let mut executor = ReviewExecutor::Snapshot;
                show(
                    changeset,
                    options,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    &mut executor,
                )
            }
            ReviewKind::Reloadable { source, mode } => run_reloadable(source, mode, options),
        }
    }

    #[cfg(test)]
    fn capabilities(&self) -> ReviewCapabilities {
        match &self.kind {
            ReviewKind::Snapshot(_) => ReviewCapabilities::SNAPSHOT,
            ReviewKind::Reloadable {
                mode: ReloadableMode::ReadOnly,
                ..
            } => ReviewCapabilities::READ_ONLY,
            ReviewKind::Reloadable {
                mode: ReloadableMode::Staged { watch, .. },
                ..
            } => ReviewCapabilities::staged(*watch),
            ReviewKind::Reloadable {
                mode: ReloadableMode::Worktree { watch, .. },
                ..
            } => ReviewCapabilities::worktree(*watch),
        }
    }
}

fn run_reloadable(
    source: &dyn DiffSource,
    mode: ReloadableMode,
    options: &ReviewOptions,
) -> ExitCode {
    let changeset = match source.load() {
        Ok(changeset) => changeset,
        Err(err) => {
            eprintln!("margin: {err}");
            return ExitCode::from(2);
        }
    };

    let diff_id = source.id().0;
    let viewed_store = viewed::ViewedStore::open(diff_id.clone());
    let viewed_entries = viewed_store
        .as_ref()
        .map(viewed::ViewedStore::load)
        .unwrap_or_default();
    let notes_store = notes::NotesStore::open(diff_id);
    let note_entries = notes_store
        .as_ref()
        .map(notes::NotesStore::load)
        .unwrap_or_default();

    // The watcher must remain alive through `show`; dropping it stops events.
    let watch_repo = match &mode {
        ReloadableMode::ReadOnly => None,
        ReloadableMode::Staged { repo, watch } | ReloadableMode::Worktree { repo, watch, .. } => {
            watch.then_some(repo.as_path())
        }
    };
    let (watch_handle, _watcher) = match watch_repo {
        Some(repo) => match start_watcher(repo) {
            Ok((handle, watcher)) => (Some(handle), Some(watcher)),
            Err(err) => {
                eprintln!("margin: --watch failed to start: {err}");
                return ExitCode::from(2);
            }
        },
        None => (None, None),
    };

    let live = LiveReview {
        source,
        persistence: Persistence {
            viewed: viewed_store,
            notes: notes_store,
        },
    };
    let mut executor = match mode {
        ReloadableMode::ReadOnly => ReviewExecutor::ReadOnly(live),
        ReloadableMode::Staged { repo, .. } => ReviewExecutor::Staged { live, repo },
        ReloadableMode::Worktree {
            repo,
            backup_discards,
            ..
        } => ReviewExecutor::Worktree {
            live,
            repo,
            backup_discards,
        },
    };
    let staged = executor.staged_summary();
    show(
        changeset,
        options,
        staged,
        watch_handle.as_deref(),
        viewed_entries,
        note_entries,
        &mut executor,
    )
}

struct Persistence {
    viewed: Option<viewed::ViewedStore>,
    notes: Option<notes::NotesStore>,
}

impl Persistence {
    #[cfg(test)]
    const fn none() -> Self {
        Self {
            viewed: None,
            notes: None,
        }
    }
}

struct LiveReview<'a> {
    source: &'a dyn DiffSource,
    persistence: Persistence,
}

enum ReviewExecutor<'a> {
    Snapshot,
    ReadOnly(LiveReview<'a>),
    Staged {
        live: LiveReview<'a>,
        repo: PathBuf,
    },
    Worktree {
        live: LiveReview<'a>,
        repo: PathBuf,
        backup_discards: bool,
    },
}

impl ReviewExecutor<'_> {
    fn live(&self) -> Option<&LiveReview<'_>> {
        match self {
            Self::Snapshot => None,
            Self::ReadOnly(live) | Self::Staged { live, .. } | Self::Worktree { live, .. } => {
                Some(live)
            }
        }
    }

    fn source(&self) -> Option<&dyn DiffSource> {
        self.live().map(|live| live.source)
    }

    fn apply_target(&self) -> Option<(&Path, &dyn DiffSource)> {
        match self {
            Self::Staged { live, repo } | Self::Worktree { live, repo, .. } => {
                Some((repo, live.source))
            }
            Self::Snapshot | Self::ReadOnly(_) => None,
        }
    }

    fn discard_target(&self) -> Option<(&Path, &dyn DiffSource, bool)> {
        match self {
            Self::Worktree {
                live,
                repo,
                backup_discards,
            } => Some((repo, live.source, *backup_discards)),
            Self::Snapshot | Self::ReadOnly(_) | Self::Staged { .. } => None,
        }
    }

    /// Staged dots are meaningful only beside a worktree review.
    fn staged_summary(&self) -> Option<margin_tui::StagedFiles> {
        match self {
            Self::Worktree { repo, .. } => Some(load_staged(repo)),
            Self::Snapshot | Self::ReadOnly(_) | Self::Staged { .. } => None,
        }
    }
}

impl CommandExecutor for ReviewExecutor<'_> {
    fn execute(&mut self, command: Command) -> CommandResult {
        match command {
            Command::ApplyHunk { action, patch } => {
                let Some((repo, source)) = self.apply_target() else {
                    return CommandResult::Unsupported(
                        "staging needs a git worktree or --staged review",
                    );
                };
                match apply_patch_to_index(repo, &patch) {
                    Ok(()) => match source.load() {
                        Ok(changeset) => CommandResult::Applied {
                            action,
                            changeset,
                            staged: self.staged_summary(),
                        },
                        Err(err) => {
                            CommandResult::Failed(format!("applied, but reload failed: {err}"))
                        }
                    },
                    Err(StageError::Stale(_)) => CommandResult::Stale(action),
                    Err(err) => CommandResult::Failed(err.to_string()),
                }
            }
            Command::DiscardHunk { backup, patch } => {
                let Some((repo, source, backup_discards)) = self.discard_target() else {
                    return CommandResult::Unsupported("discard needs a git worktree review");
                };
                // ADR-0014: nothing is destroyed before a copy exists.
                let trash_entry = if backup_discards {
                    match write_trash(repo, &backup) {
                        Ok(path) => Some(path),
                        Err(err) => {
                            return CommandResult::Failed(format!(
                                "discard aborted, backup failed: {err}"
                            ));
                        }
                    }
                } else {
                    None
                };
                match apply_patch_to_worktree(repo, &patch) {
                    Ok(()) => match source.load() {
                        Ok(changeset) => CommandResult::Discarded {
                            changeset,
                            staged: self.staged_summary(),
                            backed_up: trash_entry.is_some(),
                        },
                        Err(err) => {
                            CommandResult::Failed(format!("discarded, but reload failed: {err}"))
                        }
                    },
                    Err(StageError::Stale(_)) => {
                        // A refused dry run changed nothing; remove its orphan backup.
                        if let Some(path) = trash_entry {
                            let _ = std::fs::remove_file(path);
                        }
                        CommandResult::Stale(margin_tui::HunkAction::Discard)
                    }
                    Err(err) => CommandResult::Failed(err.to_string()),
                }
            }
            Command::SaveViewed { entries } => {
                if let Some(store) = self
                    .live()
                    .and_then(|live| live.persistence.viewed.as_ref())
                {
                    let _ = store.save(&entries);
                }
                CommandResult::Done
            }
            Command::SaveNotes { entries } => {
                if let Some(store) = self.live().and_then(|live| live.persistence.notes.as_ref()) {
                    let _ = store.save(&entries);
                }
                CommandResult::Done
            }
            Command::Reload => {
                let Some(source) = self.source() else {
                    return CommandResult::Unsupported("cannot reload patch or piped input");
                };
                match source.load() {
                    Ok(changeset) => CommandResult::Reloaded {
                        changeset,
                        staged: self.staged_summary(),
                    },
                    Err(err) => CommandResult::Failed(format!("reload failed: {err}")),
                }
            }
        }
    }
}

/// Start the OS file watcher on the repository's working-tree root.
fn start_watcher(
    repo: &Path,
) -> Result<
    (
        std::sync::Arc<margin_tui::WatchHandle>,
        notify::RecommendedWatcher,
    ),
    String,
> {
    use notify::Watcher as _;
    let root = margin_vcs::workdir_root(repo).map_err(|e| e.to_string())?;
    let handle = std::sync::Arc::new(margin_tui::WatchHandle::new(
        std::time::Duration::from_millis(250),
    ));
    let signal = std::sync::Arc::clone(&handle);
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            if event.paths.iter().any(|p| watch_relevant(p)) {
                signal.notify();
            }
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&root, notify::RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    Ok((handle, watcher))
}

fn watch_relevant(path: &Path) -> bool {
    let comps: Vec<&std::ffi::OsStr> = path.components().map(|c| c.as_os_str()).collect();
    match comps.iter().position(|c| *c == ".git") {
        None => true,
        Some(i) => {
            let rest = &comps[i + 1..];
            rest == [std::ffi::OsStr::new("index")]
                || rest == [std::ffi::OsStr::new("HEAD")]
                || rest == [std::ffi::OsStr::new("logs"), std::ffi::OsStr::new("HEAD")]
        }
    }
}

fn load_staged(repo: &Path) -> margin_tui::StagedFiles {
    margin_vcs::staged_paths(repo)
        .map(margin_tui::StagedFiles::from_paths)
        .unwrap_or_default()
}

fn show(
    changeset: Changeset,
    options: &ReviewOptions,
    staged: Option<margin_tui::StagedFiles>,
    watch: Option<&margin_tui::WatchHandle>,
    viewed: Vec<(Vec<u8>, u64)>,
    review_notes: Vec<(Vec<u8>, u64, String)>,
    executor: &mut dyn CommandExecutor,
) -> ExitCode {
    if options.json {
        match serde_json::to_string(&margin_core::json_changeset(&changeset)) {
            Ok(doc) => {
                println!("{doc}");
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("margin: cannot serialize changeset: {err}");
                return ExitCode::from(2);
            }
        }
    }
    if options.notes {
        print!("{}", margin_core::notes_markdown(&changeset, &review_notes));
        return ExitCode::SUCCESS;
    }
    if !std::io::stdout().is_terminal() {
        print_summary(&changeset);
        return ExitCode::SUCCESS;
    }

    let mut state = AppState::new(changeset);
    state.apply_theme(options.theme.clone());
    state.set_layout_mode(options.config.layout.into());
    state.set_collapse_globs(options.config.collapse.clone());
    state.set_viewed(viewed);
    state.set_notes(review_notes);
    state.staged = staged;
    state.watching = watch.is_some();
    match margin_tui::run(&mut state, executor, watch, options.config.mouse) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("margin: terminal error: {err}");
            ExitCode::from(2)
        }
    }
}

fn print_summary(changeset: &Changeset) {
    if changeset.is_empty() {
        println!("no changes");
        return;
    }
    for file in &changeset.files {
        let glyph = match file.status {
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Modified => "M",
            FileStatus::Renamed => "R",
            FileStatus::Copied => "C",
        };
        let binary = if file.is_binary { "  (binary)" } else { "" };
        println!(
            "{glyph} {:<40} +{:<4} -{:<4}{binary}",
            file.display_path(),
            file.additions(),
            file.deletions()
        );
    }
    println!(
        "{} files, +{} -{}",
        changeset.files.len(),
        changeset.additions(),
        changeset.deletions()
    );
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct ReviewCapabilities {
    reload: bool,
    watch: bool,
    watching: bool,
    stage: bool,
    discard: bool,
    persist: bool,
    staged_summary: bool,
}

#[cfg(test)]
impl ReviewCapabilities {
    const SNAPSHOT: Self = Self {
        reload: false,
        watch: false,
        watching: false,
        stage: false,
        discard: false,
        persist: false,
        staged_summary: false,
    };
    const READ_ONLY: Self = Self {
        reload: true,
        watch: false,
        watching: false,
        stage: false,
        discard: false,
        persist: true,
        staged_summary: false,
    };

    const fn staged(watching: bool) -> Self {
        Self {
            reload: true,
            watch: true,
            watching,
            stage: true,
            discard: false,
            persist: true,
            staged_summary: false,
        }
    }

    const fn worktree(watching: bool) -> Self {
        Self {
            reload: true,
            watch: true,
            watching,
            stage: true,
            discard: true,
            persist: true,
            staged_summary: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use margin_core::Changeset;
    use margin_tui::{Command, CommandExecutor, CommandResult};
    use margin_vcs::{DiffId, SourceError};

    use super::*;

    struct CountingSource {
        loads: Cell<usize>,
    }

    impl CountingSource {
        fn new() -> Self {
            Self {
                loads: Cell::new(0),
            }
        }
    }

    impl DiffSource for CountingSource {
        fn load(&self) -> Result<Changeset, SourceError> {
            self.loads.set(self.loads.get() + 1);
            Ok(Changeset::default())
        }

        fn id(&self) -> DiffId {
            DiffId("test:counting".into())
        }
    }

    #[test]
    fn constructors_encode_capabilities_for_every_review_mode() {
        let source = CountingSource::new();
        assert_eq!(
            ReviewSession::snapshot(Changeset::default()).capabilities(),
            ReviewCapabilities::SNAPSHOT
        );
        assert_eq!(
            ReviewSession::read_only(&source).capabilities(),
            ReviewCapabilities::READ_ONLY
        );
        assert_eq!(
            ReviewSession::staged(&source, PathBuf::from("repo"), true).capabilities(),
            ReviewCapabilities::staged(true)
        );
        assert_eq!(
            ReviewSession::worktree(&source, PathBuf::from("repo"), false, true).capabilities(),
            ReviewCapabilities::worktree(false)
        );
    }

    fn live(source: &dyn DiffSource) -> LiveReview<'_> {
        LiveReview {
            source,
            persistence: Persistence::none(),
        }
    }

    #[test]
    fn reload_is_available_for_every_live_mode_and_not_snapshots() {
        let source = CountingSource::new();
        let mut snapshot = ReviewExecutor::Snapshot;
        assert!(matches!(
            snapshot.execute(Command::Reload),
            CommandResult::Unsupported("cannot reload patch or piped input")
        ));

        let mut read_only = ReviewExecutor::ReadOnly(live(&source));
        assert!(matches!(
            read_only.execute(Command::Reload),
            CommandResult::Reloaded { .. }
        ));

        let mut staged = ReviewExecutor::Staged {
            live: live(&source),
            repo: PathBuf::from("not-a-repo"),
        };
        assert!(matches!(
            staged.execute(Command::Reload),
            CommandResult::Reloaded { staged: None, .. }
        ));

        let mut worktree = ReviewExecutor::Worktree {
            live: live(&source),
            repo: PathBuf::from("not-a-repo"),
            backup_discards: true,
        };
        assert!(matches!(
            worktree.execute(Command::Reload),
            CommandResult::Reloaded {
                staged: Some(_),
                ..
            }
        ));
        assert_eq!(source.loads.get(), 3);
    }

    #[test]
    fn unsupported_actions_come_from_the_review_mode() {
        let source = CountingSource::new();
        let command = || Command::ApplyHunk {
            action: margin_tui::HunkAction::Stage,
            patch: Vec::new(),
        };

        let mut snapshot = ReviewExecutor::Snapshot;
        assert!(matches!(
            snapshot.execute(command()),
            CommandResult::Unsupported("staging needs a git worktree or --staged review")
        ));

        let mut read_only = ReviewExecutor::ReadOnly(live(&source));
        assert!(matches!(
            read_only.execute(command()),
            CommandResult::Unsupported("staging needs a git worktree or --staged review")
        ));

        let mut staged = ReviewExecutor::Staged {
            live: live(&source),
            repo: PathBuf::from("not-a-repo"),
        };
        assert!(matches!(
            staged.execute(Command::DiscardHunk {
                backup: Vec::new(),
                patch: Vec::new(),
            }),
            CommandResult::Unsupported("discard needs a git worktree review")
        ));
    }

    #[test]
    fn watch_filter_passes_worktree_and_index_ignores_git_internals() {
        assert!(watch_relevant(Path::new("/repo/src/main.rs")));
        assert!(watch_relevant(Path::new("/repo/.git/index")), "staging");
        assert!(
            watch_relevant(Path::new("/repo/.git/HEAD")),
            "branch switch"
        );
        assert!(watch_relevant(Path::new("/repo/.git/logs/HEAD")), "commit");
        assert!(!watch_relevant(Path::new("/repo/.git/objects/ab/cdef")));
        assert!(!watch_relevant(Path::new("/repo/.git/index.lock")));
        assert!(!watch_relevant(Path::new("/repo/.git/refs/heads/main")));
        assert!(!watch_relevant(Path::new(
            "/repo/.git/margin/trash/1.patch"
        )));
    }
}
