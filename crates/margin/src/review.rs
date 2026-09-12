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
    apply_patch_to_index, discard_hunk, DiffSource, DiscardError, DiscardMode, DiscardOutcome,
    StageError,
};

use crate::config::Config;
use crate::review_state::{LoadedReviewState, ReviewStore};

/// Presentation and behavior settings shared by every Review Session.
pub(crate) struct ReviewOptions {
    config: Config,
    theme: Theme,
    json: bool,
    notes: bool,
    discard_without_backup: bool,
}

impl ReviewOptions {
    pub(crate) fn new(
        config: Config,
        theme: Theme,
        json: bool,
        notes: bool,
        discard_without_backup: bool,
    ) -> Self {
        Self {
            config,
            theme,
            json,
            notes,
            discard_without_backup,
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

    pub(crate) fn discard_without_backup(&self) -> bool {
        self.discard_without_backup
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
        /// Config-level intent (`discard_trash`); the invocation flag in
        /// `ReviewOptions` refines it into a `DiscardMode` at startup.
        backup_discards: bool,
    },
}

/// Resolve the effective discard behavior for a worktree review
/// (ADR-0017): backups are the default, `--discard-without-backup` opts
/// out for this invocation only, and the deprecated persistent
/// `discard_trash = false` still opts out but warns loudly on every
/// invocation until its removal. Returns the transaction mode plus a
/// startup warning, if any.
pub(crate) fn resolve_discard(
    config_backups: bool,
    flag_unbacked: bool,
) -> (DiscardMode, Option<String>) {
    if flag_unbacked {
        (DiscardMode::SkipBackup, None)
    } else if config_backups {
        (DiscardMode::BackUp, None)
    } else {
        (
            DiscardMode::SkipBackup,
            Some(
                "discard_trash = false is deprecated and will be removed: discards in this \
                 review have NO backup and cannot be undone — pass --discard-without-backup \
                 per invocation instead"
                    .to_string(),
            ),
        )
    }
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
                    Startup {
                        persisted: LoadedReviewState::default(),
                        discard_warning: None,
                        // Snapshots cannot discard; the safe default.
                        discard_backup: true,
                    },
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
    let mut review_store = ReviewStore::open(diff_id);
    let persisted: LoadedReviewState = review_store
        .as_mut()
        .map(ReviewStore::load)
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
            store: review_store,
        },
    };
    // Resolve the effective discard behavior now: backups by default, the
    // invocation-only escape hatch on request, and a loud warning for the
    // deprecated persistent opt-out — it can never be silently inherited
    // (ADR-0017).
    let mut discard_warning = None;
    let mut executor = match mode {
        ReloadableMode::ReadOnly => ReviewExecutor::ReadOnly(live),
        ReloadableMode::Staged { repo, .. } => ReviewExecutor::Staged { live, repo },
        ReloadableMode::Worktree {
            repo,
            backup_discards: config_backups,
            ..
        } => {
            let (discard, warning) =
                resolve_discard(config_backups, options.discard_without_backup());
            discard_warning = warning;
            ReviewExecutor::Worktree {
                live,
                repo,
                discard,
            }
        }
    };
    let staged = executor.staged_summary();
    // The prompt states the consequence before anything is typed; only a
    // worktree review can discard, so anything else keeps the safe default.
    let discard_backup = executor
        .discard_target()
        .is_none_or(|(_, _, mode)| mode == DiscardMode::BackUp);
    show(
        changeset,
        options,
        staged,
        watch_handle.as_deref(),
        Startup {
            persisted,
            discard_warning,
            discard_backup,
        },
        &mut executor,
    )
}

struct Persistence {
    store: Option<ReviewStore>,
}

impl Persistence {
    #[cfg(test)]
    const fn none() -> Self {
        Self { store: None }
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
        discard: DiscardMode,
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

    fn discard_target(&self) -> Option<(&Path, &dyn DiffSource, DiscardMode)> {
        match self {
            Self::Worktree {
                live,
                repo,
                discard,
            } => Some((repo, live.source, *discard)),
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
                let Some((repo, source, mode)) = self.discard_target() else {
                    return CommandResult::Unsupported("discard needs a git worktree review");
                };
                // One recoverable transaction owns backup, preflight,
                // apply, and failure handling (ADR-0014, issue #96); the
                // outcome is reported verbatim, never reinterpreted.
                let outcome = match discard_hunk(repo, &backup, &patch, mode) {
                    Ok(outcome) => outcome,
                    Err(DiscardError::Stale(_)) => {
                        return CommandResult::Stale(margin_tui::HunkAction::Discard);
                    }
                    Err(err) => return CommandResult::Failed(err.to_string()),
                };
                match source.load() {
                    Ok(changeset) => CommandResult::Discarded {
                        changeset,
                        staged: self.staged_summary(),
                        recovery: match outcome {
                            DiscardOutcome::BackedUp => margin_tui::DiscardRecovery::BackedUp,
                            DiscardOutcome::Unbacked => margin_tui::DiscardRecovery::Unbacked,
                        },
                    },
                    Err(err) => {
                        CommandResult::Failed(format!("discarded, but reload failed: {err}"))
                    }
                }
            }
            Command::SaveReviewState { viewed, notes } => {
                if let Some(store) = self.live().and_then(|live| live.persistence.store.as_ref()) {
                    match store.save(&viewed, &notes) {
                        Ok(()) => CommandResult::Done,
                        Err(err) => CommandResult::Failed(format!(
                            "review state not saved ({err}); marks and notes are session-only"
                        )),
                    }
                } else {
                    CommandResult::Done
                }
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

/// Everything a review needs beyond the changeset itself: recovered
/// review state, the discard consequence to promise in the prompt, and
/// any startup notices to show without blocking the review.
struct Startup {
    persisted: LoadedReviewState,
    discard_warning: Option<String>,
    discard_backup: bool,
}

fn show(
    changeset: Changeset,
    options: &ReviewOptions,
    staged: Option<margin_tui::StagedFiles>,
    watch: Option<&margin_tui::WatchHandle>,
    startup: Startup,
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
        print!(
            "{}",
            margin_core::notes_markdown(&changeset, &startup.persisted.notes)
        );
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
    state.set_viewed(startup.persisted.viewed);
    state.set_notes(startup.persisted.notes);
    // Startup notices are conveniences, never blockers: a damaged review
    // record or a deprecated discard opt-out starts the review anyway,
    // with the reasons visible until the next keypress.
    state.status_message = match (startup.persisted.warning, startup.discard_warning) {
        (Some(first), Some(second)) => Some(format!("{first} — {second}")),
        (warning, None) | (None, warning) => warning,
    };
    // The prompt states the consequence before anything is typed
    // (ADR-0017); only a worktree review can discard.
    state.discard_backup = startup.discard_backup;
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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
    fn discard_policy_defaults_to_backups_with_no_warning() {
        let (mode, warning) = resolve_discard(true, false);
        assert_eq!(mode, DiscardMode::BackUp);
        assert!(warning.is_none());
    }

    #[test]
    fn discard_policy_flag_opts_out_for_this_invocation_only() {
        let (mode, warning) = resolve_discard(true, true);
        assert_eq!(mode, DiscardMode::SkipBackup);
        assert!(warning.is_none(), "an explicit choice needs no warning");
    }

    #[test]
    fn discard_policy_deprecated_config_warns_every_time() {
        // ADR-0017: the persistent opt-out still works until removal,
        // but no invocation may silently inherit it.
        let (mode, warning) = resolve_discard(false, false);
        assert_eq!(mode, DiscardMode::SkipBackup);
        let warning = warning.expect("deprecated opt-out must warn");
        assert!(warning.contains("deprecated"), "{warning}");
        assert!(warning.contains("--discard-without-backup"), "{warning}");
    }

    #[test]
    fn discard_policy_flag_wins_over_deprecated_config() {
        let (mode, warning) = resolve_discard(false, true);
        assert_eq!(mode, DiscardMode::SkipBackup);
        assert!(
            warning.is_none(),
            "explicit choice supersedes the legacy setting"
        );
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
            discard: DiscardMode::BackUp,
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
