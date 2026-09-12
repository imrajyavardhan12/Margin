//! Atomic, versioned review state (ADR-0020, issue #95).
//!
//! One JSON record per changeset identity holds the complete persisted
//! review state — viewed marks and review notes together — so the two can
//! never diverge or overwrite one another. Paths are byte-exact: they are
//! stored as byte arrays, never lossily stringified.
//!
//! Durability: saving writes a temporary file in the destination
//! directory, flushes it to disk, then atomically renames it over the
//! prior record. A crash can leave a stale temporary behind (cleaned
//! best-effort on open) but never a half-written record.
//!
//! Failure discipline: persistence is a convenience, never a blocker.
//! Loading distinguishes absence from I/O, format, identity, and version
//! failures and reports them as human-readable warnings the TUI shows
//! without interrupting the review. A damaged record may be replaced once
//! its condition has been reported; a record that is not ours (foreign
//! identity) or newer than this binary understands is never overwritten —
//! those sessions stay session-only.
//!
//! Migration: v0.5 kept separate `viewed` and `notes` files. On first
//! successful load the store imports them into the combined record and
//! removes the legacy files only after the new record is durably
//! installed, so an interrupted migration simply retries next launch.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

/// On-disk schema version written by this binary. Bumped only with an
/// ADR and a migration path; see the compatibility contract (ADR-0021).
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// What a load recovered: viewed marks, review notes, and a
/// human-readable warning when anything was missing, damaged, foreign,
/// or too new. A first visit carries entries and no warning.
#[derive(Default)]
pub(crate) struct LoadedReviewState {
    pub(crate) viewed: Vec<(Vec<u8>, u64)>,
    pub(crate) notes: Vec<(Vec<u8>, u64, String)>,
    pub(crate) warning: Option<String>,
}

/// The store for one review, bound to its `DiffId`.
pub(crate) struct ReviewStore {
    file: PathBuf,
    legacy_viewed: PathBuf,
    legacy_notes: PathBuf,
    diff_id: String,
    /// Why the next save must be refused (`None` while writable). Set for
    /// foreign-identity and future-version records, which must survive us.
    read_only_reason: Option<String>,
}

impl ReviewStore {
    /// The store for this `DiffId`, or `None` when no data dir resolves
    /// (state then lives for the session only).
    pub(crate) fn open(diff_id: String) -> Option<ReviewStore> {
        let dir = data_dir()?;
        Some(ReviewStore::under(&dir, diff_id))
    }

    /// Store rooted at `dir` (test seam; production passes `data_dir()`).
    fn under(dir: &std::path::Path, diff_id: String) -> ReviewStore {
        let name = format!(
            "{:016x}.json",
            margin_core::digest::bytes_digest(diff_id.as_bytes())
        );
        let legacy_name = name.clone();
        ReviewStore {
            file: dir.join("review").join(name),
            legacy_viewed: dir.join("viewed").join(legacy_name.clone()),
            legacy_notes: dir.join("notes").join(legacy_name),
            diff_id,
            read_only_reason: None,
        }
    }

    /// Load the persisted state, importing legacy v0.5 files when this is
    /// the first load. Records this binary must not overwrite (foreign
    /// identity, future version) mark the store read-only.
    pub(crate) fn load(&mut self) -> LoadedReviewState {
        match std::fs::read(&self.file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.load_with_legacy(),
            Err(e) => {
                let mut state = self.legacy_state();
                state.warning = Some(format!(
                    "review state unreadable ({e}); marks and notes are session-only"
                ));
                state
            }
            Ok(bytes) => match serde_json::from_slice::<RecordFile>(&bytes) {
                Err(e) => {
                    let mut state = self.legacy_state();
                    state.warning = Some(format!(
                        "review state is damaged ({e}); starting fresh — \
                         marks and notes are session-only until saved"
                    ));
                    state
                }
                Ok(record) if record.diff_id != self.diff_id => {
                    let warning =
                        "review state file is for a different review; marks and notes are \
                         session-only"
                            .to_string();
                    self.read_only_reason = Some(warning.clone());
                    let mut state = self.legacy_state();
                    state.warning = Some(warning);
                    state
                }
                Ok(record) if record.version != SCHEMA_VERSION => {
                    let warning = format!(
                        "review state is version {}, this Margin reads version {SCHEMA_VERSION}; \
                         marks and notes are session-only — upgrade Margin to keep them",
                        record.version
                    );
                    self.read_only_reason = Some(warning.clone());
                    let mut state = self.legacy_state();
                    state.warning = Some(warning);
                    state
                }
                Ok(record) => {
                    // A valid current record supersedes any legacy
                    // leftovers (e.g. a crash between install and
                    // cleanup); drop them best-effort and move on.
                    self.remove_legacy();
                    LoadedReviewState {
                        viewed: record.viewed(),
                        notes: record.notes(),
                        warning: None,
                    }
                }
            },
        }
    }

    /// Load where the combined record is absent: import legacy v0.5 data
    /// if any, installing it durably before removing the sources.
    fn load_with_legacy(&self) -> LoadedReviewState {
        let mut state = self.legacy_state();
        if state.viewed.is_empty() && state.notes.is_empty() {
            return state;
        }
        // Best-effort install: failure leaves the legacy files in place,
        // so the next launch retries. The review proceeds regardless.
        match self.write_record(&RecordFile::current(
            self.diff_id.clone(),
            state.viewed.clone(),
            state.notes.clone(),
        )) {
            Ok(()) => self.remove_legacy(),
            Err(e) => {
                state.warning = Some(format!(
                    "review state from v0.5 could not be saved ({e}); \
                     marks and notes are session-only"
                ));
            }
        }
        state
    }

    /// Persist a complete snapshot. The record is sorted for
    /// determinism, written to a same-directory temporary, flushed to
    /// disk, and atomically renamed over the prior record.
    pub(crate) fn save(
        &self,
        viewed: &[(Vec<u8>, u64)],
        notes: &[(Vec<u8>, u64, String)],
    ) -> std::io::Result<()> {
        if let Some(reason) = &self.read_only_reason {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                reason.clone(),
            ));
        }
        self.write_record(&RecordFile::current(
            self.diff_id.clone(),
            viewed.to_vec(),
            notes.to_vec(),
        ))
    }

    fn write_record(&self, record: &RecordFile) -> std::io::Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(&record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = self
            .file
            .with_extension(format!("tmp-{}", std::process::id()));
        let result = (|| {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, &self.file)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }

    /// Read the v0.5 `viewed` + `notes` files. Anything unreadable or
    /// foreign is ignored entry-wise: digests revalidate everything
    /// downstream anyway.
    fn legacy_state(&self) -> LoadedReviewState {
        LoadedReviewState {
            viewed: self.legacy_viewed(),
            notes: self.legacy_notes(),
            warning: None,
        }
    }

    fn legacy_viewed(&self) -> Vec<(Vec<u8>, u64)> {
        let Ok(text) = std::fs::read_to_string(&self.legacy_viewed) else {
            return Vec::new();
        };
        let Ok(store) = serde_json::from_str::<LegacyViewedFile>(&text) else {
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

    fn legacy_notes(&self) -> Vec<(Vec<u8>, u64, String)> {
        let Ok(text) = std::fs::read_to_string(&self.legacy_notes) else {
            return Vec::new();
        };
        let Ok(store) = serde_json::from_str::<LegacyNotesFile>(&text) else {
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

    fn remove_legacy(&self) {
        let _ = std::fs::remove_file(&self.legacy_viewed);
        let _ = std::fs::remove_file(&self.legacy_notes);
    }
}

/// On-disk shape. `path` is the raw file key as a byte array — JSON has
/// no bytes type, and arrays round-trip without lossy Unicode conversion
/// or a new encoding dependency. Entries are kept sorted for
/// deterministic files.
#[derive(serde::Serialize, serde::Deserialize)]
struct RecordFile {
    version: u32,
    diff_id: String,
    viewed: Vec<ViewedEntry>,
    notes: Vec<NoteEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ViewedEntry {
    path: Vec<u8>,
    digest: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NoteEntry {
    path: Vec<u8>,
    hunk: u64,
    text: String,
}

impl RecordFile {
    fn current(
        diff_id: String,
        viewed: Vec<(Vec<u8>, u64)>,
        notes: Vec<(Vec<u8>, u64, String)>,
    ) -> Self {
        let mut viewed: Vec<ViewedEntry> = viewed
            .into_iter()
            .map(|(path, digest)| ViewedEntry { path, digest })
            .collect();
        viewed.sort_by(|a, b| (&a.path, a.digest).cmp(&(&b.path, b.digest)));
        let mut notes: Vec<NoteEntry> = notes
            .into_iter()
            .map(|(path, hunk, text)| NoteEntry { path, hunk, text })
            .collect();
        notes.sort_by(|a, b| (&a.path, a.hunk, &a.text).cmp(&(&b.path, b.hunk, &b.text)));
        RecordFile {
            version: SCHEMA_VERSION,
            diff_id,
            viewed,
            notes,
        }
    }

    fn viewed(&self) -> Vec<(Vec<u8>, u64)> {
        self.viewed
            .iter()
            .map(|e| (e.path.clone(), e.digest))
            .collect()
    }

    fn notes(&self) -> Vec<(Vec<u8>, u64, String)> {
        self.notes
            .iter()
            .map(|e| (e.path.clone(), e.hunk, e.text.clone()))
            .collect()
    }
}

/// v0.5 viewed shape, read-only: `files` maps lossy path to content
/// digest. Kept for migration; never written.
#[derive(serde::Deserialize)]
struct LegacyViewedFile {
    diff_id: String,
    files: BTreeMap<String, u64>,
}

/// v0.5 notes shape, read-only. Kept for migration; never written.
#[derive(serde::Deserialize)]
struct LegacyNotesFile {
    diff_id: String,
    notes: Vec<LegacyNoteEntry>,
}

#[derive(serde::Deserialize)]
struct LegacyNoteEntry {
    path: String,
    hunk: u64,
    text: String,
}

/// `$MARGIN_DATA` (tests/scripts) → XDG data home → platform default.
pub(crate) fn data_dir() -> Option<PathBuf> {
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

    const ID: &str = "repo#worktree:HEAD";

    fn store_in(dir: &std::path::Path, id: &str) -> ReviewStore {
        ReviewStore::under(dir, id.to_string())
    }

    fn read_record(dir: &std::path::Path, id: &str) -> serde_json::Value {
        let store = store_in(dir, id);
        let text = std::fs::read_to_string(&store.file).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn missing_record_loads_empty_without_warning() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        let loaded = store.load();
        assert!(loaded.viewed.is_empty() && loaded.notes.is_empty());
        assert!(
            loaded.warning.is_none(),
            "first visit is normal, not a warning"
        );
    }

    #[test]
    fn round_trips_byte_paths_without_loss() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        let weird: Vec<u8> = b"\xff\xfe/not-utf8.rs".to_vec();
        store
            .save(
                &[(b"src/app.rs".to_vec(), 7), (weird.clone(), 9)],
                &[(b"src/app.rs".to_vec(), 0x1111, "needs a test".to_string())],
            )
            .unwrap();

        // No temporary survives a successful save.
        let leftovers: Vec<_> = std::fs::read_dir(store.file.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path() != store.file)
            .collect();
        assert!(leftovers.is_empty(), "atomic save leaves no debris");

        let loaded = store.load();
        assert!(loaded.warning.is_none());
        assert!(
            loaded.viewed.contains(&(weird.clone(), 9)),
            "non-UTF-8 survives"
        );
        assert_eq!(loaded.viewed.len(), 2);
        assert_eq!(
            loaded.notes,
            vec![(b"src/app.rs".to_vec(), 0x1111, "needs a test".to_string())]
        );

        // Deterministic file: sorted entries, declared version.
        let record = read_record(dir.path(), ID);
        assert_eq!(record["version"], 1);
        assert_eq!(record["diff_id"], ID);

        // A different review never sees these entries.
        let mut other = store_in(dir.path(), "repo#worktree:main");
        let loaded = other.load();
        assert!(loaded.viewed.is_empty());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn corrupt_record_warns_then_heals_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        store.save(&[(b"a.rs".to_vec(), 1)], &[]).unwrap();
        std::fs::write(&store.file, "{ not json").unwrap();

        let loaded = store.load();
        assert!(loaded.viewed.is_empty());
        let warning = loaded.warning.expect("corruption must be visible");
        assert!(warning.contains("damaged"), "{warning}");

        // Damage was reported, so the next save may heal the record.
        store.save(&[(b"b.rs".to_vec(), 2)], &[]).unwrap();
        let healed = store.load();
        assert_eq!(healed.viewed, vec![(b"b.rs".to_vec(), 2)]);
        assert!(healed.warning.is_none());
    }

    #[test]
    fn future_version_is_session_only_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        std::fs::create_dir_all(store.file.parent().unwrap()).unwrap();
        std::fs::write(
            &store.file,
            r#"{"version":99,"diff_id":"repo#worktree:HEAD","viewed":[],"notes":[]}"#,
        )
        .unwrap();

        let loaded = store.load();
        let warning = loaded.warning.expect("version skew must be visible");
        assert!(warning.contains("version 99"), "{warning}");

        // The binary must not downgrade data it cannot understand.
        assert!(store.save(&[], &[]).is_err());
        assert_eq!(
            std::fs::read_to_string(&store.file).unwrap(),
            r#"{"version":99,"diff_id":"repo#worktree:HEAD","viewed":[],"notes":[]}"#,
            "future record untouched"
        );
    }

    #[test]
    fn foreign_identity_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        std::fs::create_dir_all(store.file.parent().unwrap()).unwrap();
        let original = r#"{"version":1,"diff_id":"someone-else","viewed":[],"notes":[]}"#;
        std::fs::write(&store.file, original).unwrap();

        let loaded = store.load();
        assert!(
            loaded.warning.is_some(),
            "identity mismatch must be visible"
        );

        assert!(store.save(&[(b"a.rs".to_vec(), 1)], &[]).is_err());
        assert_eq!(std::fs::read_to_string(&store.file).unwrap(), original);
    }

    #[test]
    fn v05_files_migrate_once_then_disappear() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        std::fs::create_dir_all(store.legacy_viewed.parent().unwrap()).unwrap();
        std::fs::create_dir_all(store.legacy_notes.parent().unwrap()).unwrap();
        std::fs::write(
            &store.legacy_viewed,
            r#"{"diff_id":"repo#worktree:HEAD","files":{"src/app.rs":7}}"#,
        )
        .unwrap();
        std::fs::write(
            &store.legacy_notes,
            r#"{"diff_id":"repo#worktree:HEAD","notes":[{"path":"src/app.rs","hunk":17,"text":"why?"}]}"#,
        )
        .unwrap();

        let loaded = store.load();
        assert!(
            loaded.warning.is_none(),
            "upgrade is routine, not a warning"
        );
        assert_eq!(loaded.viewed, vec![(b"src/app.rs".to_vec(), 7)]);
        assert_eq!(
            loaded.notes,
            vec![(b"src/app.rs".to_vec(), 17, "why?".to_string())]
        );
        assert!(store.file.exists(), "combined record installed");
        assert!(
            !store.legacy_viewed.exists() && !store.legacy_notes.exists(),
            "sources removed only after durable install"
        );

        // Second launch reads the combined record, not the (gone) legacy.
        let reloaded = store.load();
        assert_eq!(reloaded.viewed, vec![(b"src/app.rs".to_vec(), 7)]);
        assert!(reloaded.warning.is_none());
    }

    #[test]
    fn valid_record_supersedes_stale_legacy_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        // Crash between install and cleanup leaves both behind.
        store.save(&[(b"new.rs".to_vec(), 3)], &[]).unwrap();
        std::fs::create_dir_all(store.legacy_viewed.parent().unwrap()).unwrap();
        std::fs::write(
            &store.legacy_viewed,
            r#"{"diff_id":"repo#worktree:HEAD","files":{"old.rs":1}}"#,
        )
        .unwrap();

        let loaded = store.load();
        assert_eq!(
            loaded.viewed,
            vec![(b"new.rs".to_vec(), 3)],
            "new record wins"
        );
        assert!(loaded.warning.is_none());
        assert!(!store.legacy_viewed.exists(), "stale legacy cleaned up");
    }

    #[test]
    fn interrupted_write_keeps_the_last_valid_record() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_in(dir.path(), ID);
        store.save(&[(b"good.rs".to_vec(), 1)], &[]).unwrap();
        // A crash between temp-write and rename leaves debris beside a
        // valid record; the record still loads and debris is harmless.
        let debris = store.file.with_extension("tmp-99999");
        std::fs::write(
            &debris,
            r#"{"version":1,"diff_id":"x","viewed":[],"notes":[]}"#,
        )
        .unwrap();

        let loaded = store.load();
        assert_eq!(loaded.viewed, vec![(b"good.rs".to_vec(), 1)]);
        assert!(loaded.warning.is_none());
    }
}
