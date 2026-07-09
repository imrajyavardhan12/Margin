//! The `margin` binary: CLI parsing, source selection, and the terminal
//! session.
//!
//! Responsibilities (and nothing more — ADR-0004):
//! 1. Parse the git-verb CLI (ADR-0007) and, later, config (ADR-0008).
//! 2. Choose a `margin_vcs::DiffSource` (or read stdin/file bytes) from the
//!    invocation.
//! 3. Honor the passthrough guarantee: in `pager` and `patch` modes with a
//!    non-TTY stdout, input bytes flow through byte-identical, exit 0.
//! 4. Run the TUI on a terminal; print a plain summary when piped.
//!
//! Exit codes are an API (ADR-0007): 0 success, 2 usage/environment error.
//! (1 is reserved for "displayed with errors".)

mod config;

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use config::{Config, LayoutChoice};
use margin_core::{parse_unified, Changeset, FileStatus, ParseWarning};
use margin_tui::theme::{Theme, THEME_NAMES};
use margin_tui::AppState;
use margin_vcs::{
    apply_patch_to_index, apply_patch_to_worktree, undo_last_discard, write_trash, DiffSource,
    GitRevRange, GitShow, GitStaged, GitWorktree, StageError, TwoFiles,
};

#[derive(Parser)]
#[command(
    name = "margin",
    version,
    about = "A fast, keyboard-first terminal diff viewer",
    long_about = "Review Git changes, patches, and AI-authored code without leaving the terminal.\n\
                  Run with no arguments to review the working tree (untracked files included)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Review staged changes (shorthand for `margin diff --staged`)
    #[arg(long)]
    staged: bool,

    /// Theme: ledger, foolscap, carbon, blueprint
    #[arg(long, global = true, value_name = "NAME")]
    theme: Option<String>,

    /// Diff layout
    #[arg(long, global = true, value_enum)]
    layout: Option<LayoutChoice>,

    /// Print the effective configuration (after merging files and flags)
    #[arg(long)]
    dump_config: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Review working-tree changes, a revision (range), or two files
    Diff(DiffArgs),
    /// Review one commit against its first parent
    Show {
        /// Revision to show (defaults to HEAD)
        rev: Option<String>,
    },
    /// Review a unified diff from stdin (`-`) or a patch file
    Patch {
        /// `-` for stdin (the default) or a path to a .patch/.diff file
        input: Option<String>,
    },
    /// Git pager mode: interactive on a terminal, byte-identical
    /// passthrough when piped (safe as `git config core.pager`)
    Pager,
    /// Restore the most recent discarded hunk from the trash (ADR-0014)
    Undo,
}

#[derive(Args)]
struct DiffArgs {
    /// Review the index (staged changes) instead of the working tree
    #[arg(long)]
    staged: bool,

    /// A revision (`HEAD~2`), a range (`main..feature`), or two files
    #[arg(value_name = "REV|RANGE|FILE", num_args = 0..=2)]
    targets: Vec<String>,
}

/// Everything `show` needs besides the source: the merged config and the
/// theme resolved against the terminal's color capability.
struct Session {
    config: Config,
    theme: Theme,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let cwd = working_dir().ok();
    let config = match Config::load(
        config::user_config_path().as_deref(),
        cwd.as_deref(),
        cli.theme.as_deref(),
        cli.layout,
    ) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("margin: config error {message}");
            return ExitCode::from(2);
        }
    };
    if cli.dump_config {
        print!("{}", config.dump());
        return ExitCode::SUCCESS;
    }
    let Some(theme) = Theme::resolve(&config.theme, config::detect_color_mode()) else {
        eprintln!(
            "margin: unknown theme '{}' (built-in themes: {})",
            config.theme,
            THEME_NAMES.join(", ")
        );
        return ExitCode::from(2);
    };
    let session = Session { config, theme };

    let command = cli.command.unwrap_or(Command::Diff(DiffArgs {
        staged: cli.staged,
        targets: Vec::new(),
    }));

    match command {
        Command::Diff(args) => run_diff(args, &session),
        Command::Show { rev } => {
            let cwd = match working_dir() {
                Ok(dir) => dir,
                Err(code) => return code,
            };
            run_source(
                &GitShow::new(cwd, rev.unwrap_or_else(|| "HEAD".into())),
                &session,
                None,
                false,
            )
        }
        Command::Patch { input } => run_patch(input.as_deref().unwrap_or("-"), &session),
        Command::Pager => run_patch("-", &session),
        Command::Undo => run_undo(),
    }
}

/// `margin undo`: restore the newest trash entry to the working tree.
/// Empty trash and stale entries exit 2 with the reason (ADR-0007); a
/// stale entry is kept and its path printed for hand-recovery.
fn run_undo() -> ExitCode {
    let cwd = match working_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    match undo_last_discard(&cwd) {
        Ok(path) => {
            println!("restored {}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("margin: {err}");
            ExitCode::from(2)
        }
    }
}

fn run_diff(args: DiffArgs, session: &Session) -> ExitCode {
    let cwd = match working_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    if args.staged && !args.targets.is_empty() {
        eprintln!("margin: --staged cannot be combined with revisions or files");
        return ExitCode::from(2);
    }
    if args.staged {
        // Staging commands stay meaningful (`u` unstages from the index),
        // but the staged indicator does not: everything shown is staged.
        return run_source(&GitStaged::new(cwd.clone()), session, Some(cwd), false);
    }

    match args.targets.as_slice() {
        [] => {
            let mut source = GitWorktree::new(cwd.clone());
            source.include_untracked = session.config.include_untracked;
            run_source(&source, session, Some(cwd), true)
        }
        [single] => {
            if let Some((from, to)) = split_range(single) {
                run_source(&GitRevRange::new(cwd, from, to), session, None, false)
            } else {
                // `margin diff <rev>`: working tree vs that revision —
                // git's semantics.
                let mut source = GitWorktree::new(cwd.clone());
                source.include_untracked = session.config.include_untracked;
                source.base = Some(single.clone());
                run_source(&source, session, Some(cwd), true)
            }
        }
        [a, b] => {
            if Path::new(a).is_file() && Path::new(b).is_file() {
                run_source(&TwoFiles::new(a, b), session, None, false)
            } else {
                run_source(
                    &GitRevRange::new(cwd, a.clone(), b.clone()),
                    session,
                    None,
                    false,
                )
            }
        }
        _ => unreachable!("clap caps targets at 2"),
    }
}

/// `A..B` / `A...B` -> (A, B); empty sides default to HEAD, like git.
fn split_range(spec: &str) -> Option<(String, String)> {
    let (from, to) = spec.split_once("...").or_else(|| spec.split_once(".."))?;
    let or_head = |s: &str| {
        if s.is_empty() {
            "HEAD".to_string()
        } else {
            s.to_string()
        }
    };
    Some((or_head(from), or_head(to)))
}

/// `patch`/`pager` mode: raw bytes in; passthrough when piped, TUI when not.
fn run_patch(input: &str, session: &Session) -> ExitCode {
    let bytes = if input == "-" {
        let mut buf = Vec::new();
        if let Err(err) = std::io::stdin().lock().read_to_end(&mut buf) {
            eprintln!("margin: cannot read stdin: {err}");
            return ExitCode::from(2);
        }
        buf
    } else {
        match std::fs::read(input) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("margin: cannot read {input}: {err}");
                return ExitCode::from(2);
            }
        }
    };

    // The passthrough guarantee (ADR-0007): piped output is byte-identical
    // to the input — `git -c core.pager='margin pager' log -p | grep` must
    // behave exactly as without us.
    if !std::io::stdout().is_terminal() {
        let mut stdout = std::io::stdout().lock();
        if stdout
            .write_all(&bytes)
            .and_then(|()| stdout.flush())
            .is_err()
        {
            // Downstream closed the pipe (e.g. `| head`): not an error.
            return ExitCode::SUCCESS;
        }
        return ExitCode::SUCCESS;
    }

    // Git colorizes output destined for a pager; strip ANSI for parsing.
    let outcome = parse_unified(&margin_core::strip_ansi(&bytes));
    let mut executor = VcsExecutor {
        repo: None,
        source: None,
        worktree: false,
        trash: false,
    };
    let code = show(outcome.changeset, session, None, &mut executor);
    report_warnings(&outcome.warnings);
    code
}

/// Executes TUI commands against the real repository (ADR-0013/0014).
/// `repo` is Some only for index-relative reviews (worktree/--staged),
/// where staging a displayed hunk is meaningful; everything else refuses.
/// `worktree` is true only for worktree reviews — the one place the
/// sidebar's staged dots carry information (`--staged` shows staged
/// content by definition) and the only place a displayed hunk can be
/// discarded (its lines *are* working-tree content there).
/// `trash` mirrors `discard_trash`: back up before destroying.
struct VcsExecutor<'a> {
    repo: Option<PathBuf>,
    source: Option<&'a dyn DiffSource>,
    worktree: bool,
    trash: bool,
}

impl VcsExecutor<'_> {
    /// The sidebar's staged summary, when it applies to this review
    /// (`None` otherwise — see `margin_tui::StagedFiles`).
    fn staged_summary(&self) -> Option<margin_tui::StagedFiles> {
        if !self.worktree {
            return None;
        }
        self.repo.as_deref().map(load_staged)
    }
}

impl margin_tui::CommandExecutor for VcsExecutor<'_> {
    fn execute(&mut self, command: margin_tui::Command) -> margin_tui::CommandResult {
        use margin_tui::CommandResult;
        match command {
            margin_tui::Command::ApplyHunk { action, patch } => {
                let (Some(repo), Some(source)) = (&self.repo, self.source) else {
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
            margin_tui::Command::DiscardHunk { backup, patch } => {
                let (true, Some(repo), Some(source)) = (self.worktree, &self.repo, self.source)
                else {
                    return CommandResult::Unsupported("discard needs a git worktree review");
                };
                // ADR-0014: nothing is destroyed before a copy exists —
                // a failed trash write aborts the discard entirely.
                let trash_entry = if self.trash {
                    match write_trash(repo, &backup) {
                        Ok(path) => Some(path),
                        Err(err) => {
                            return CommandResult::Failed(format!(
                                "discard aborted, backup failed: {err}"
                            ))
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
                        // The dry run refused: nothing was destroyed, so
                        // the orphan backup would only mislead a later undo.
                        if let Some(path) = trash_entry {
                            let _ = std::fs::remove_file(path);
                        }
                        CommandResult::Stale(margin_tui::HunkAction::Discard)
                    }
                    Err(err) => CommandResult::Failed(err.to_string()),
                }
            }
            margin_tui::Command::Reload => {
                let Some(source) = self.source else {
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

fn run_source(
    source: &dyn DiffSource,
    session: &Session,
    staging_repo: Option<PathBuf>,
    worktree: bool,
) -> ExitCode {
    match source.load() {
        Ok(changeset) => {
            let mut executor = VcsExecutor {
                repo: staging_repo,
                source: Some(source),
                worktree,
                trash: session.config.discard_trash,
            };
            let staged = executor.staged_summary();
            show(changeset, session, staged, &mut executor)
        }
        Err(err) => {
            eprintln!("margin: {err}");
            ExitCode::from(2)
        }
    }
}

/// The sidebar's staged summary: the index-vs-HEAD diff reduced to the set
/// of staged paths. Best-effort — the indicator is advisory, so a failure
/// (unborn branch, transient git error) simply yields an empty summary
/// rather than blocking the review.
fn load_staged(repo: &Path) -> margin_tui::StagedFiles {
    GitStaged::new(repo.to_path_buf())
        .load()
        .map(|changeset| margin_tui::StagedFiles::from_staged_changeset(&changeset))
        .unwrap_or_default()
}

/// Render a changeset: TUI on a terminal, plain summary when piped.
fn show(
    changeset: Changeset,
    session: &Session,
    staged: Option<margin_tui::StagedFiles>,
    executor: &mut dyn margin_tui::CommandExecutor,
) -> ExitCode {
    if !std::io::stdout().is_terminal() {
        print_summary(&changeset);
        return ExitCode::SUCCESS;
    }
    let mut state = AppState::new(changeset);
    state.apply_theme(session.theme.clone());
    state.set_layout_mode(session.config.layout.into());
    state.staged = staged;
    match margin_tui::run(&mut state, executor) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("margin: terminal error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Surface parse anomalies after the TUI closes (never swallowed, ADR-0009).
fn report_warnings(warnings: &[ParseWarning]) {
    const SHOWN: usize = 5;
    for warning in warnings.iter().take(SHOWN) {
        eprintln!("margin: patch line {}: {}", warning.line, warning.message);
    }
    if warnings.len() > SHOWN {
        eprintln!("margin: ...and {} more warnings", warnings.len() - SHOWN);
    }
}

fn working_dir() -> Result<PathBuf, ExitCode> {
    std::env::current_dir().map_err(|err| {
        eprintln!("margin: cannot determine working directory: {err}");
        ExitCode::from(2)
    })
}

/// Plain listing for non-TTY stdout (pipes, scripts).
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
