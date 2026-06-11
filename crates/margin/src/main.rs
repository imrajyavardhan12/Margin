//! The `margin` binary: CLI parsing, config discovery, source selection,
//! and the terminal session.
//!
//! Responsibilities (and nothing more — ADR-0004):
//! 1. Parse CLI args (full clap surface arrives with issue #5) and config
//!    (ADR-0007, ADR-0008).
//! 2. Choose a `margin_vcs::DiffSource` from the invocation.
//! 3. Run the TUI when stdout is a terminal; print a plain summary when it
//!    is not (precursor of the pager passthrough guarantee, ADR-0007).
//!
//! Exit codes per ADR-0007: 0 success, 2 usage/environment error.

use std::io::IsTerminal;
use std::process::ExitCode;

use margin_core::{Changeset, FileStatus};
use margin_tui::AppState;
use margin_vcs::{DiffSource, GitStaged, GitWorktree};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("margin {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let staged = args.iter().any(|a| a == "--staged");

    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("margin: cannot determine working directory: {err}");
            return ExitCode::from(2);
        }
    };

    let source: Box<dyn DiffSource> = if staged {
        Box::new(GitStaged::new(&cwd))
    } else {
        Box::new(GitWorktree::new(&cwd))
    };

    let changeset = match source.load() {
        Ok(changeset) => changeset,
        Err(err) => {
            eprintln!("margin: {err}");
            return ExitCode::from(2);
        }
    };

    if !std::io::stdout().is_terminal() {
        print_summary(&changeset);
        return ExitCode::SUCCESS;
    }

    let mut state = AppState::new(changeset);
    match margin_tui::run(&mut state) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("margin: terminal error: {err}");
            ExitCode::from(2)
        }
    }
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
