//! GitHub pull requests through the user's `gh` CLI (ADR-0015,
//! issue #24).
//!
//! Margin never holds a token or opens a connection: `gh` — which the
//! user has already authenticated — does the forge work. This module is
//! the only place a subprocess is spawned, and it stays quarantined here
//! exactly like git2 does (ADR-0005). The diff bytes come from
//! `gh pr diff` and go through the same tolerant parser as stdin patches.

use std::path::PathBuf;
use std::process::Command;

use margin_core::{parse_unified, Changeset};

use crate::{DiffId, DiffSource, SourceError};

/// One pull request, resolved to a canonical identity at construction.
pub struct GhPr {
    /// What the user typed: a number, a `#123`, a branch, or a URL —
    /// anything `gh` accepts.
    selector: String,
    /// Where `gh` should run (PR selectors resolve against the repo's
    /// GitHub remote).
    cwd: PathBuf,
    /// The PR's canonical URL, from `gh pr view` — the stable identity
    /// that keys viewed-state across sessions and force-pushes.
    url: String,
}

impl GhPr {
    /// Resolve `selector` against the repository at `cwd`. Runs
    /// `gh pr view` once; a missing `gh` or an unresolvable PR errors
    /// here, before any terminal state exists.
    pub fn resolve(
        cwd: impl Into<PathBuf>,
        selector: impl Into<String>,
    ) -> Result<Self, SourceError> {
        let cwd = cwd.into();
        let selector = selector.into();
        // Argument-injection guard: the selector is user input handed to
        // an authenticated CLI's argv. A leading `-` would be parsed by
        // gh as a flag (`-R other/repo`, `--web`), not a selector — and
        // no legitimate selector (number, #123, branch, URL) starts with
        // one. Both invocations also place the selector after `--`
        // (end-of-flags), so even a validation gap cannot smuggle flags.
        if selector.starts_with('-') || selector.is_empty() {
            return Err(SourceError::Git(format!(
                "'{selector}' is not a pull request selector (expected a number, branch, or URL)"
            )));
        }
        let output = run_gh(
            &cwd,
            &[
                "pr", "view", "--json", "url", "--jq", ".url", "--", &selector,
            ],
        )?;
        let url = String::from_utf8_lossy(&output).trim().to_string();
        if url.is_empty() {
            return Err(SourceError::Git(format!(
                "gh could not resolve pull request '{selector}'"
            )));
        }
        Ok(GhPr { selector, cwd, url })
    }

    /// The canonical PR URL (shown in the error/UI surface).
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl DiffSource for GhPr {
    fn load(&self) -> Result<Changeset, SourceError> {
        let bytes = run_gh(&self.cwd, &["pr", "diff", "--", &self.selector])?;
        // gh emits clean git-generated diffs; the tolerant parser's
        // warnings would only fire on transport corruption, where the
        // partial parse is still the most useful thing to show.
        Ok(parse_unified(&bytes).changeset)
    }

    fn id(&self) -> DiffId {
        // Deliberately NOT the head SHA (ADR-0015): per-file content
        // digests already invalidate viewed marks precisely, so untouched
        // files stay checked across force-pushes.
        DiffId(format!("gh-pr:{}", self.url))
    }
}

/// Run `gh` and return stdout. A missing binary and a nonzero exit get
/// distinct, user-actionable errors — the latter passes `gh`'s own
/// stderr through (auth guidance, 404s) rather than rewording it.
fn run_gh(cwd: &std::path::Path, args: &[&str]) -> Result<Vec<u8>, SourceError> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                SourceError::Git(
                    "margin pr needs the GitHub CLI (gh) on PATH — https://cli.github.com".into(),
                )
            } else {
                SourceError::Git(format!("cannot run gh: {err}"))
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SourceError::Git(format!(
            "gh {}: {}",
            args.first().copied().unwrap_or(""),
            stderr.trim()
        )));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The injection guard fires before any subprocess: flag-shaped
    /// selectors must never reach the authenticated gh's argv.
    #[test]
    fn flag_shaped_selectors_are_rejected_without_running_gh() {
        for hostile in ["-R", "--web", "--repo evil/repo", "-", ""] {
            let err = match GhPr::resolve("/nonexistent-dir", hostile) {
                Err(err) => err.to_string(),
                Ok(_) => panic!("{hostile:?} must be rejected"),
            };
            assert!(
                err.contains("not a pull request selector"),
                "{hostile:?}: {err}"
            );
        }
    }
}
