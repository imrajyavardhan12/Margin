//! Diffing two arbitrary files — `margin diff old.rs new.rs`.
//! No repository involved: git2's buffer diffing gives us the same hunk
//! quality (and binary detection) as the repo-backed sources.

use std::path::PathBuf;

use git2::DiffOptions;
use margin_core::{Changeset, FileDiff, FileStatus};

use crate::git::{convert_hunks, file_from_delta, map_status};
use crate::{DiffId, DiffSource, SourceError};

pub struct TwoFiles {
    pub old: PathBuf,
    pub new: PathBuf,
}

impl TwoFiles {
    pub fn new(old: impl Into<PathBuf>, new: impl Into<PathBuf>) -> Self {
        Self {
            old: old.into(),
            new: new.into(),
        }
    }
}

impl DiffSource for TwoFiles {
    fn load(&self) -> Result<Changeset, SourceError> {
        let read = |path: &PathBuf| {
            std::fs::read(path).map_err(|e| SourceError::Io {
                path: path.clone(),
                message: e.to_string(),
            })
        };
        let old_bytes = read(&self.old)?;
        let new_bytes = read(&self.new)?;

        let mut opts = DiffOptions::new();
        let patch = git2::Patch::from_buffers(
            &old_bytes,
            Some(&self.old),
            &new_bytes,
            Some(&self.new),
            Some(&mut opts),
        )?;

        let delta = patch.delta();
        let status = map_status(delta.status()).unwrap_or(FileStatus::Modified);
        let mut file: FileDiff = file_from_delta(&delta, status);
        file.is_binary = delta.flags().is_binary() || file.is_binary;
        file.hunks = convert_hunks(&patch)?;

        // Identical files: an empty changeset, not a zero-hunk entry.
        let changed = (file.is_binary && old_bytes != new_bytes) || !file.hunks.is_empty();
        Ok(Changeset {
            files: if changed { vec![file] } else { Vec::new() },
        })
    }

    fn id(&self) -> DiffId {
        DiffId(format!(
            "files:{}..{}",
            self.old.display(),
            self.new.display()
        ))
    }
}
