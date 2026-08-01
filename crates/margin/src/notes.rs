//! Persistent review notes (issue #23): one JSON file per `DiffId` under
//! the data dir, beside the viewed marks. Same discipline as
//! [`crate::viewed`] — every failure path degrades to session-only notes,
//! because losing the review you are *in* to a disk error would be worse
//! than losing the note.

use std::path::PathBuf;

/// The note store for one review, bound to its `DiffId`.
pub struct NotesStore {
    file: PathBuf,
    diff_id: String,
}

/// On-disk shape. A `Vec` (not a map) because the key is a pair, and
/// entries arrive already sorted, so the JSON stays deterministic.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoreFile {
    diff_id: String,
    notes: Vec<Entry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    /// Lossy display path — advisory; the digest does the real matching.
    path: String,
    /// Digest of the hunk this note belongs to.
    hunk: u64,
    text: String,
}

impl NotesStore {
    /// The store for this `DiffId`, or `None` when no data dir resolves
    /// (notes then live for the session only).
    pub fn open(diff_id: String) -> Option<NotesStore> {
        let name = format!(
            "{:016x}.json",
            margin_core::digest::bytes_digest(diff_id.as_bytes())
        );
        Some(NotesStore {
            file: crate::viewed::data_dir()?.join("notes").join(name),
            diff_id,
        })
    }

    /// Load the notes as `(path bytes, hunk digest, text)`. Any failure —
    /// missing file, bad JSON, foreign diff_id — yields nothing; digests
    /// revalidate whatever survives.
    pub fn load(&self) -> Vec<(Vec<u8>, u64, String)> {
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
            .notes
            .into_iter()
            .map(|e| (e.path.into_bytes(), e.hunk, e.text))
            .collect()
    }

    /// Persist the notes; errors are the caller's to ignore.
    pub fn save(&self, entries: &[(String, u64, String)]) -> std::io::Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = StoreFile {
            diff_id: self.diff_id.clone(),
            notes: entries
                .iter()
                .map(|(path, hunk, text)| Entry {
                    path: path.clone(),
                    hunk: *hunk,
                    text: text.clone(),
                })
                .collect(),
        };
        let text = serde_json::to_string(&store)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.file, text)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn store_in(dir: &std::path::Path, id: &str) -> NotesStore {
        // Bypass the env var to keep tests parallel-safe.
        let name = format!(
            "{:016x}.json",
            margin_core::digest::bytes_digest(id.as_bytes())
        );
        NotesStore {
            file: dir.join("notes").join(name),
            diff_id: id.to_string(),
        }
    }

    #[test]
    fn round_trips_and_isolates_by_diff_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), "git:worktree:/repo");
        assert!(store.load().is_empty(), "no file yet");

        let entries = vec![
            ("src/a.rs".to_string(), 0x1111, "needs a test".to_string()),
            ("src/b.rs".to_string(), 0x2222, "why unwrap?".to_string()),
        ];
        store.save(&entries).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].0, b"src/a.rs".to_vec());
        assert_eq!(loaded[0].1, 0x1111);
        assert_eq!(loaded[0].2, "needs a test");

        // A different review sharing the directory sees nothing of ours.
        let other = store_in(dir.path(), "gh-pr:https://example/pr/1");
        assert!(other.load().is_empty());
    }

    #[test]
    fn corrupt_store_degrades_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), "id");
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::write(&store.file, "{ not json").unwrap();
        assert!(store.load().is_empty(), "a bad store never blocks a review");
    }
}
