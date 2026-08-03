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
mod notes;
mod review;
mod viewed;

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand};
use config::{Config, LayoutChoice};
use margin_core::{parse_unified, ParseWarning};
use margin_tui::theme::{Theme, THEME_NAMES};
use margin_vcs::{undo_last_discard, GhPr, GitRevRange, GitShow, GitStaged, GitWorktree, TwoFiles};
use review::{ReviewOptions, ReviewSession};

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

    /// Reload automatically when the repository changes (debounced)
    #[arg(short = 'w', long)]
    watch: bool,

    /// Emit the changeset as JSON (schema 1) instead of opening the TUI
    #[arg(long)]
    json: bool,

    /// Print this review's notes as Markdown instead of opening the TUI
    /// (issue #23) — paste into a pull request or hand to an agent
    #[arg(long, global = true)]
    notes: bool,

    /// Theme: auto (match the terminal background), ledger, foolscap,
    /// carbon, blueprint
    #[arg(long, global = true, value_name = "NAME")]
    theme: Option<String>,

    /// Diff layout
    #[arg(long, global = true, value_enum)]
    layout: Option<LayoutChoice>,

    /// Exclude untracked files from worktree reviews
    #[arg(long, global = true)]
    no_untracked: bool,

    /// Disable mouse capture (keeps the terminal's own text selection)
    #[arg(long, global = true)]
    no_mouse: bool,

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
        /// Emit the changeset as JSON (schema 1)
        #[arg(long)]
        json: bool,
    },
    /// Review a unified diff from stdin (`-`) or a patch file
    Patch {
        /// `-` for stdin (the default) or a path to a .patch/.diff file
        input: Option<String>,
        /// Emit the changeset as JSON (schema 1)
        #[arg(long)]
        json: bool,
    },
    /// Review a GitHub pull request via the authenticated `gh` CLI
    /// (ADR-0015): a number, branch, or URL — anything `gh` accepts
    Pr {
        /// PR number, branch name, or URL
        selector: String,
        /// Emit the changeset as JSON (schema 1)
        #[arg(long)]
        json: bool,
    },
    /// Git pager mode: interactive on a terminal, byte-identical
    /// passthrough when piped (safe as `git config core.pager`)
    Pager,
    /// Restore the most recent discarded hunk from the trash (ADR-0014)
    Undo,
    /// Print shell completions to stdout (ADR-0016) — e.g.
    /// `margin completions zsh > "$fpath[1]/_margin"`
    Completions {
        /// Shell dialect to emit
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Print the roff man page to stdout (`margin man | man -l -`) —
    /// for packagers; regular users read `--help` (ADR-0016)
    #[command(hide = true)]
    Man,
}

#[derive(Args)]
struct DiffArgs {
    /// Review the index (staged changes) instead of the working tree
    #[arg(long)]
    staged: bool,

    /// Reload automatically when the repository changes (debounced)
    #[arg(short = 'w', long)]
    watch: bool,

    /// Emit the changeset as JSON (schema 1) instead of opening the TUI
    #[arg(long)]
    json: bool,

    /// A revision (`HEAD~2`), a range (`main..feature`), or two files
    #[arg(value_name = "REV|RANGE|FILE", num_args = 0..=2)]
    targets: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Completions and the man page dispatch before config: completions
    // are eval'd from shell rc files, and a typo in config.toml must
    // never break shell startup (ADR-0016).
    match cli.command {
        Some(Command::Completions { shell }) => {
            // Render to a buffer first: clap_complete panics on a write
            // error, and `margin completions zsh | head` must not panic.
            let mut script = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "margin", &mut script);
            return write_stdout(&script);
        }
        Some(Command::Man) => return run_man(),
        _ => {}
    }

    let cwd = working_dir().ok();
    let config = match Config::load(
        config::user_config_path().as_deref(),
        cwd.as_deref(),
        cli.theme.as_deref(),
        cli.layout,
        cli.no_untracked,
        cli.no_mouse,
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
    // "auto" resolves to a real theme name here, before Theme::resolve —
    // the TUI never knows detection happened (issue #27). Config wins:
    // any explicit theme skips the terminal query entirely.
    let theme_name = if config.theme == "auto" {
        config::auto_theme_name().to_string()
    } else {
        config.theme.clone()
    };
    // Custom themes first (issue #15): a `[themes.ledger]` deliberately
    // shadows the built-in, which also lets `auto` pick up the tweak.
    let mode = config::detect_color_mode();
    let theme = if let Some(custom) = config.themes.get(&theme_name) {
        match custom.build(&theme_name, mode) {
            Ok(theme) => theme,
            Err(message) => {
                eprintln!("margin: config error {message}");
                return ExitCode::from(2);
            }
        }
    } else if let Some(theme) = Theme::resolve(&theme_name, mode) {
        theme
    } else {
        eprintln!(
            "margin: unknown theme '{theme_name}' (auto, built-in themes: {}, or a [themes.<name>] from your config)",
            THEME_NAMES.join(", ")
        );
        return ExitCode::from(2);
    };
    let command = cli.command.unwrap_or(Command::Diff(DiffArgs {
        staged: cli.staged,
        watch: cli.watch,
        json: cli.json,
        targets: Vec::new(),
    }));

    // Pager mode never emits JSON: its piped output is byte-identical by
    // contract (ADR-0007), and interactively it is a review, not a query.
    let json = match &command {
        Command::Diff(args) => cli.json || args.json,
        Command::Show { json, .. } | Command::Patch { json, .. } | Command::Pr { json, .. } => {
            cli.json || *json
        }
        Command::Pager | Command::Undo | Command::Completions { .. } | Command::Man => false,
    };
    // Both flags ask for a document instead of a review; picking one
    // silently would be a guess (ADR-0007: refuse loudly).
    if json && cli.notes {
        eprintln!("margin: --json and --notes cannot be combined");
        return ExitCode::from(2);
    }
    let session = ReviewOptions::new(config, theme, json, cli.notes);

    match command {
        Command::Diff(args) => run_diff(args, &session),
        Command::Show { rev, .. } => {
            let cwd = match working_dir() {
                Ok(dir) => dir,
                Err(code) => return code,
            };
            ReviewSession::read_only(&GitShow::new(cwd, rev.unwrap_or_else(|| "HEAD".into())))
                .run(&session)
        }
        Command::Patch { input, .. } => run_patch(input.as_deref().unwrap_or("-"), &session),
        Command::Pr { selector, .. } => {
            let cwd = match working_dir() {
                Ok(dir) => dir,
                Err(code) => return code,
            };
            match GhPr::resolve(cwd, selector) {
                Ok(source) => ReviewSession::read_only(&source).run(&session),
                Err(err) => {
                    eprintln!("margin: {err}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Pager => run_patch("-", &session),
        Command::Undo => run_undo(),
        Command::Completions { .. } | Command::Man => {
            unreachable!("dispatched before config load")
        }
    }
}

/// `margin man`: the roff page to stdout, for packagers (ADR-0016).
fn run_man() -> ExitCode {
    let mut page = Vec::new();
    if let Err(err) = clap_mangen::Man::new(Cli::command()).render(&mut page) {
        eprintln!("margin: cannot render man page: {err}");
        return ExitCode::from(2);
    }
    write_stdout(&page)
}

/// Write generated text to stdout. A closed pipe (`| head`, a pager
/// quitting) is normal use, not an error; anything else is exit 2.
fn write_stdout(bytes: &[u8]) -> ExitCode {
    match std::io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("margin: {err}");
            ExitCode::from(2)
        }
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

fn run_diff(args: DiffArgs, session: &ReviewOptions) -> ExitCode {
    let cwd = match working_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    if args.staged && !args.targets.is_empty() {
        eprintln!("margin: --staged cannot be combined with revisions or files");
        return ExitCode::from(2);
    }
    if args.watch && session.json_output() {
        eprintln!("margin: --watch and --json cannot be combined");
        return ExitCode::from(2);
    }
    if args.staged {
        // Staging commands stay meaningful (`u` unstages from the index),
        // but the staged indicator does not: everything shown is staged.
        return ReviewSession::staged(&GitStaged::new(cwd.clone()), cwd, args.watch).run(session);
    }

    match args.targets.as_slice() {
        [] => {
            let mut source = GitWorktree::new(cwd.clone());
            source.include_untracked = session.include_untracked();
            ReviewSession::worktree(&source, cwd, args.watch, session.discard_backups())
                .run(session)
        }
        [single] => {
            if let Some((from, to)) = split_range(single) {
                if args.watch {
                    return watch_needs_worktree();
                }
                ReviewSession::read_only(&GitRevRange::new(cwd, from, to)).run(session)
            } else {
                // `margin diff <rev>`: working tree vs that revision —
                // git's semantics.
                let mut source = GitWorktree::new(cwd.clone());
                source.include_untracked = session.include_untracked();
                source.base = Some(single.clone());
                ReviewSession::worktree(&source, cwd, args.watch, session.discard_backups())
                    .run(session)
            }
        }
        [a, b] => {
            if args.watch {
                return watch_needs_worktree();
            }
            if Path::new(a).is_file() && Path::new(b).is_file() {
                ReviewSession::read_only(&TwoFiles::new(a, b)).run(session)
            } else {
                ReviewSession::read_only(&GitRevRange::new(cwd, a.clone(), b.clone())).run(session)
            }
        }
        _ => unreachable!("clap caps targets at 2"),
    }
}

/// Static views (ranges, two files) have nothing live to watch (ADR-0007:
/// refuse loudly, exit 2, rather than silently ignore a flag).
fn watch_needs_worktree() -> ExitCode {
    eprintln!("margin: --watch needs a worktree or --staged review");
    ExitCode::from(2)
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
fn run_patch(input: &str, session: &ReviewOptions) -> ExitCode {
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
    // behave exactly as without us. `--json` opts out: the caller asked
    // for the parsed document, piped or not (pager mode never sets it).
    if !session.json_output() && !std::io::stdout().is_terminal() {
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
    let warnings = outcome.warnings;
    let code = ReviewSession::snapshot(outcome.changeset).run(session);
    report_warnings(&warnings);
    code
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
