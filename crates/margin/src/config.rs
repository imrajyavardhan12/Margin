//! Configuration discovery and merging (ADR-0008).
//!
//! Precedence, later wins: built-in defaults -> user config
//! (`$XDG_CONFIG_HOME/margin/config.toml`, `~/.config/margin/` fallback,
//! `%APPDATA%\margin\` on Windows; `$MARGIN_CONFIG` overrides the path) ->
//! repo-local `.margin.toml` -> CLI flags.
//!
//! Trust rule (ADR-0008): the repo-local file is **display options only**
//! (`theme`, `layout`) and that is enforced by its schema — a checked-out
//! repository must never be able to change Margin's behavior. Unknown keys
//! in either file are errors with did-you-mean suggestions (serde).

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use margin_tui::app::LayoutMode;
use margin_tui::theme::ColorMode;
use serde::Deserialize;

/// Layout choice shared by config files and the `--layout` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum LayoutChoice {
    Auto,
    Unified,
    Split,
}

impl From<LayoutChoice> for LayoutMode {
    fn from(choice: LayoutChoice) -> Self {
        match choice {
            LayoutChoice::Auto => LayoutMode::Auto,
            LayoutChoice::Unified => LayoutMode::Unified,
            LayoutChoice::Split => LayoutMode::Split,
        }
    }
}

/// The user config file: full surface.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct UserFile {
    theme: Option<String>,
    layout: Option<LayoutChoice>,
    include_untracked: Option<bool>,
}

/// The repo-local `.margin.toml`: display options only, by schema (ADR-0008).
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RepoFile {
    theme: Option<String>,
    layout: Option<LayoutChoice>,
}

/// The merged result the rest of the binary consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub theme: String,
    pub layout: LayoutChoice,
    pub include_untracked: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "ledger".into(),
            layout: LayoutChoice::Auto,
            include_untracked: true,
        }
    }
}

impl Config {
    /// Load and merge: defaults <- user file <- repo file <- flags.
    /// `user_path`/`repo_dir` are injected for testability; `main` passes
    /// the real environment.
    pub fn load(
        user_path: Option<&Path>,
        repo_dir: Option<&Path>,
        flag_theme: Option<&str>,
        flag_layout: Option<LayoutChoice>,
    ) -> Result<Config, String> {
        let mut config = Config::default();

        if let Some(path) = user_path {
            if path.is_file() {
                let user: UserFile = read_toml(path)?;
                merge(&mut config.theme, user.theme);
                merge_opt(&mut config.layout, user.layout);
                merge_opt(&mut config.include_untracked, user.include_untracked);
            }
        }
        if let Some(dir) = repo_dir {
            if let Some(path) = find_repo_config(dir) {
                let repo: RepoFile = read_toml(&path)?;
                merge(&mut config.theme, repo.theme);
                merge_opt(&mut config.layout, repo.layout);
            }
        }
        if let Some(theme) = flag_theme {
            config.theme = theme.to_string();
        }
        if let Some(layout) = flag_layout {
            config.layout = layout;
        }
        Ok(config)
    }

    /// The effective config as TOML — `margin --dump-config`.
    pub fn dump(&self) -> String {
        let layout = match self.layout {
            LayoutChoice::Auto => "auto",
            LayoutChoice::Unified => "unified",
            LayoutChoice::Split => "split",
        };
        format!(
            "theme = \"{}\"\nlayout = \"{}\"\ninclude_untracked = {}\n",
            self.theme, layout, self.include_untracked
        )
    }
}

fn merge(slot: &mut String, value: Option<String>) {
    if let Some(value) = value {
        *slot = value;
    }
}

fn merge_opt<T>(slot: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *slot = value;
    }
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("in {}: {e}", path.display()))
}

/// The default user config path for this platform, honoring
/// `$MARGIN_CONFIG` (explicit file) and `$XDG_CONFIG_HOME`.
pub fn user_config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("MARGIN_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|dir| dir.join("margin").join("config.toml"))
}

/// Walk up from `start` looking for `.margin.toml`, stopping at the repo
/// boundary (a `.git` entry) or the filesystem root.
fn find_repo_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        let candidate = dir.join(".margin.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None; // repo root reached without a config
        }
        dir = dir.parent()?;
    }
}

/// Terminal color capability: `NO_COLOR` wins, then truecolor signals,
/// otherwise the 16-color-safe palette.
pub fn detect_color_mode() -> ColorMode {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return ColorMode::Monochrome;
    }
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    if colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit") {
        return ColorMode::TrueColor;
    }
    let term = std::env::var("TERM").unwrap_or_default();
    if [
        "256color",
        "kitty",
        "ghostty",
        "alacritty",
        "wezterm",
        "iterm",
    ]
    .iter()
    .any(|hint| term.contains(hint))
    {
        return ColorMode::TrueColor;
    }
    ColorMode::Ansi16
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn precedence_user_then_repo_then_flags() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config.toml");
        std::fs::write(&user, "theme = \"carbon\"\ninclude_untracked = false\n").unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".margin.toml"), "theme = \"blueprint\"\n").unwrap();

        let config = Config::load(Some(&user), Some(&repo), None, None).unwrap();
        assert_eq!(config.theme, "blueprint", "repo overrides user");
        assert!(!config.include_untracked, "user file applies");

        let config = Config::load(
            Some(&user),
            Some(&repo),
            Some("foolscap"),
            Some(LayoutChoice::Split),
        )
        .unwrap();
        assert_eq!(config.theme, "foolscap", "flags win");
        assert_eq!(config.layout, LayoutChoice::Split);
    }

    #[test]
    fn unknown_keys_error_with_suggestions() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config.toml");
        std::fs::write(&user, "them = \"carbon\"\n").unwrap();
        let err = Config::load(Some(&user), None, None, None).unwrap_err();
        assert!(err.contains("them"), "{err}");
        assert!(err.contains("theme"), "suggestion expected: {err}");
    }

    #[test]
    fn repo_config_rejects_behavior_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".margin.toml"),
            "include_untracked = false\n",
        )
        .unwrap();
        let err = Config::load(None, Some(dir.path()), None, None).unwrap_err();
        assert!(
            err.contains("include_untracked"),
            "repo config must be display-only (ADR-0008): {err}"
        );
    }

    #[test]
    fn repo_search_stops_at_git_boundary() {
        let dir = tempfile::tempdir().unwrap();
        // outer/.margin.toml exists, but inner/ is its own repo root.
        std::fs::write(dir.path().join(".margin.toml"), "theme = \"carbon\"\n").unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        assert_eq!(find_repo_config(&inner), None);

        // Without the boundary the outer file is found.
        let free = dir.path().join("free");
        std::fs::create_dir_all(&free).unwrap();
        assert_eq!(
            find_repo_config(&free),
            Some(dir.path().join(".margin.toml"))
        );
    }

    #[test]
    fn missing_files_yield_defaults() {
        let config = Config::load(
            Some(Path::new("/definitely/not/here.toml")),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn dump_round_trips_through_the_parser() {
        let dump = Config::default().dump();
        let parsed: UserFile = toml::from_str(&dump).unwrap();
        assert_eq!(parsed.theme.as_deref(), Some("ledger"));
    }
}
