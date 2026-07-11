//! Stable content digests for review state (issue #20).
//!
//! A viewed mark must outlive the process and survive rebases, so it is
//! keyed by a digest of the file's *diff content*: same hunks, same
//! digest, mark stands; any content change invalidates it. The algorithm
//! is FNV-1a (64-bit), hand-rolled because `std`'s hasher is explicitly
//! unstable across Rust releases and this value is persisted. This is
//! change-detection, not cryptography: an adversarial collision merely
//! keeps a checkmark.

use crate::model::{FileDiff, LineKind};

/// Digest of everything reviewable about a file's diff: status, paths,
/// modes, binary flag, and every hunk's geometry and line bytes.
pub fn file_digest(file: &FileDiff) -> u64 {
    let mut h = Fnv::new();
    h.byte(match file.status {
        crate::model::FileStatus::Added => 0,
        crate::model::FileStatus::Deleted => 1,
        crate::model::FileStatus::Modified => 2,
        crate::model::FileStatus::Renamed => 3,
        crate::model::FileStatus::Copied => 4,
    });
    h.opt_bytes(file.old_path.as_deref());
    h.opt_bytes(file.new_path.as_deref());
    h.u32(file.old_mode.unwrap_or(0));
    h.u32(file.new_mode.unwrap_or(0));
    h.byte(u8::from(file.is_binary));
    for hunk in &file.hunks {
        h.u32(hunk.old_start);
        h.u32(hunk.old_count);
        h.u32(hunk.new_start);
        h.u32(hunk.new_count);
        for line in &hunk.lines {
            h.byte(match line.kind {
                LineKind::Context => b' ',
                LineKind::Addition => b'+',
                LineKind::Deletion => b'-',
            });
            h.bytes(&line.content);
            h.byte(u8::from(line.no_newline));
        }
    }
    h.finish()
}

/// Digest of arbitrary bytes (same stable FNV-1a): used to derive
/// filesystem-safe store names from `DiffId` strings.
pub fn bytes_digest(bytes: &[u8]) -> u64 {
    let mut h = Fnv::new();
    h.bytes(bytes);
    h.finish()
}

/// FNV-1a over arbitrary field sequences; length-prefixes variable-size
/// values so `("ab","c")` and `("a","bc")` cannot collide structurally.
struct Fnv(u64);

impl Fnv {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Fnv(Self::OFFSET)
    }

    fn byte(&mut self, b: u8) {
        self.0 = (self.0 ^ u64::from(b)).wrapping_mul(Self::PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.u32(bytes.len() as u32);
        for &b in bytes {
            self.byte(b);
        }
    }

    fn opt_bytes(&mut self, bytes: Option<&[u8]>) {
        match bytes {
            None => self.byte(0),
            Some(b) => {
                self.byte(1);
                self.bytes(b);
            }
        }
    }

    fn u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.byte(b);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::parse_unified;

    const A: &[u8] = b"--- a/x.rs\n+++ b/x.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";

    #[test]
    fn digest_is_deterministic_and_content_sensitive() {
        let one = parse_unified(A).changeset;
        let two = parse_unified(A).changeset;
        assert_eq!(
            file_digest(&one.files[0]),
            file_digest(&two.files[0]),
            "same content, same digest"
        );

        let changed =
            parse_unified(b"--- a/x.rs\n+++ b/x.rs\n@@ -1,1 +1,1 @@\n-old\n+NEW\n").changeset;
        assert_ne!(
            file_digest(&one.files[0]),
            file_digest(&changed.files[0]),
            "content change must invalidate"
        );

        let moved =
            parse_unified(b"--- a/x.rs\n+++ b/x.rs\n@@ -9,1 +9,1 @@\n-old\n+new\n").changeset;
        assert_ne!(
            file_digest(&one.files[0]),
            file_digest(&moved.files[0]),
            "hunk position change invalidates too (rebase moved the code)"
        );
    }

    #[test]
    fn digest_value_is_pinned_across_releases() {
        // The digest is PERSISTED: if this pin ever breaks, every user's
        // viewed marks silently invalidate. Change it only deliberately.
        let file = &parse_unified(A).changeset.files[0];
        assert_eq!(file_digest(file), 0x511e_bea2_0318_2e2b);
    }
}
