//! The `margin` binary: CLI parsing, config discovery, source selection,
//! and the terminal session.
//!
//! Responsibilities (and nothing more — ADR-0004):
//! 1. Parse CLI args (clap, with issue #2) and config (ADR-0007, ADR-0008).
//! 2. Choose a `margin_vcs::DiffSource` from the invocation.
//! 3. If stdout is not a TTY in pager mode, pass input through unchanged
//!    (the "safe as core.pager" guarantee, ADR-0007).
//! 4. Otherwise run the `margin-tui` event loop.
//!
//! Current state: a walking skeleton. Until the TUI lands (issue #2) this
//! prints a textual changeset summary so the git sources are dogfoodable.
//! Exit codes already follow ADR-0007: 0 success, 2 usage/environment error.

use margin_core::FileStatus;
use margin_vcs::{DiffSource, GitStaged, GitWorktree};
use std::process::ExitCode;

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

    match source.load() {
        Ok(changeset) if changeset.is_empty() => {
            println!("no changes");
            ExitCode::SUCCESS
        }
        Ok(changeset) => {
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
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("margin: {err}");
            ExitCode::from(2)
        }
    }
}
