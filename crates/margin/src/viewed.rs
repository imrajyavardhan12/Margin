//! Persistent viewed marks (issue #20): one JSON file per `DiffId` under
//! the data dir, so quitting and relaunching the same review keeps your
//! checkmarks. Every failure path degrades to session-only marks —
//! persistence is a convenience, never a blocker.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The store for one review, bound to its `DiffId`.
pub struct ViewedStore {
    file: PathBuf,
    diff_id: String,
}

/// On-disk shape. `diff_id` is stored verbatim to guard against the
/// (astronomically unlikely) filename-hash collision; `files` is a
/// BTreeMap so the JSON is deterministic.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoreFile {
    diff_id: String,
    files: BTreeMap<String, u64>,
}

impl ViewedStore {
    /// The store for this `DiffId`, or `None` when no data dir resolves
    /// (marks then live for the session only).
    pub fn open(diff_id: String) -> Option<ViewedStore> {
        let name = format!(
            "{:016x}.json",
            margin_core::digest::bytes_digest(diff_id.as_bytes())
        );
        Some(ViewedStore {
            file: data_dir()?.join("viewed").join(name),
            diff_id,
        })
    }

    /// Load the marks. Any failure — missing file, bad JSON, foreign
    /// diff_id — yields an empty set: digests revalidate everything
    /// downstream anyway.
    pub fn load(&self) -> Vec<(Vec<u8>, u64)> {
        let Ok(text) = std::fs::read_to_string(&self.file) else {
            return Vec::new();
        };
        let Ok(store) = serde_json::from_str::<StoreFile>(&text) else {
            return Vec::new();
        };
        if store.diff_id != self.diff_id {
            return Vec::new();
        }
        store
            .files
            .into_iter()
            .map(|(path, digest)| (path.into_bytes(), digest))
            .collect()
    }

    /// Persist the marks; errors are the caller's to ignore (session-only
    /// fallback by design).
    pub fn save(&self, entries: &[(String, u64)]) -> std::io::Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = StoreFile {
            diff_id: self.diff_id.clone(),
            files: entries.iter().cloned().collect(),
        };
        let text = serde_json::to_string(&store)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.file, text)
    }
}

/// `$MARGIN_DATA` (tests/scripts) → XDG data home → platform default.
fn data_dir() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("MARGIN_DATA") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|dir| dir.join("margin"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn store_in(dir: &std::path::Path, id: &str) -> ViewedStore {
        // Bypass the env var to keep tests parallel-safe.
        let name = format!(
            "{:016x}.json",
            margin_core::digest::bytes_digest(id.as_bytes())
        );
        ViewedStore {
            file: dir.join("viewed").join(name),
            diff_id: id.to_string(),
        }
    }

    #[test]
    fn round_trips_and_isolates_by_diff_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), "repo#worktree:HEAD");
        assert!(store.load().is_empty(), "missing file is empty, not error");

        store
            .save(&[("src/app.rs".into(), 7), ("docs/x.md".into(), 9)])
            .unwrap();
        let mut loaded = store.load();
        loaded.sort();
        assert_eq!(
            loaded,
            vec![(b"docs/x.md".to_vec(), 9), (b"src/app.rs".to_vec(), 7)]
        );

        // A different review never sees these marks.
        let other = store_in(dir.path(), "repo#worktree:main");
        assert!(other.load().is_empty());

        // Corrupt file: session-only fallback, no panic.
        std::fs::write(&store.file, "not json").unwrap();
        assert!(store.load().is_empty());
    }
}
