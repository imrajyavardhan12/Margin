# ADR-0020: Persist review state as one versioned, atomic record

- **Status:** Accepted
- **Date:** 2026-08-02
- **Supersedes:** the separate `ViewedStore` and `NotesStore` persistence formats

## Context

Viewed marks and review notes are currently stored by two nearly identical
modules. Both write directly to their destination file, collapse every load
failure into an empty result, and have save failures ignored by the runtime. A
crash can therefore truncate a record and make valid review state disappear
silently. The string-based path format also conflicts with Margin's bytes-first
model on repositories containing non-UTF-8 paths.

## Decision

The binary owns one `ReviewStore` per changeset identity. Its versioned record
contains the complete persisted review state: viewed marks and review notes,
with paths represented without lossy Unicode conversion.

Saving writes a temporary file in the destination directory, flushes it, and
atomically replaces the prior record. Loading distinguishes absence from I/O,
format, identity, and version failures. Failures do not block review, but they
produce a visible warning and never cause a damaged record to be silently
overwritten.

On first successful load, the store imports the legacy viewed and notes files.
Legacy data is removed only after the combined record is durably installed, so
migration is retryable after interruption.

## Consequences

- Viewed marks and notes have one consistency and failure boundary.
- The TUI emits a complete review-state snapshot rather than independent writes
  that could overwrite one another.
- Persistence remains optional and local; patch and pager reviews can remain
  session-only.
- The on-disk version and migration behavior become user-facing compatibility
  responsibilities even though the Rust type remains private.
