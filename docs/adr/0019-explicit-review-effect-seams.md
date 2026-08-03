# ADR-0019: Explicit read, write, and runtime effect seams

- **Status:** Accepted
- **Date:** 2026-08-02
- **Amends:** ADR-0004, ADR-0005, ADR-0013, and ADR-0014

## Context

The founding ADRs describe `DiffSource` as Margin's only outside-world seam.
That was sufficient for the read-only product, but it is no longer true:
Margin now applies index and worktree changes, persists review state, watches
files, reads configuration and stdin, controls a terminal, and invokes `gh`.
The binary currently coordinates some review capabilities through independent
`Option` fields and booleans, allowing invalid combinations and distributing
discard safety across multiple modules.

## Decision

The four-crate dependency direction remains unchanged, but its effect boundaries
are made explicit:

- `DiffSource` is the read-only Interface for producing and identifying a
  changeset; it is not described as the only I/O seam.
- `margin-vcs` owns VCS and forge Adapters and exposes deep write operations
  without leaking `git2` types.
- A discard transaction in `margin-vcs` owns backup creation, checked apply,
  and failed-apply backup cleanup as one testable operation.
- A capability-aware review-session Module in the binary composes a source,
  supported write behavior, persistence, and watching without independent
  flags representing impossible states.
- `margin-tui` continues to express effects as `Command` values and consume
  results; it never imports `margin-vcs` or performs review-domain I/O.
- Configuration, review-state persistence, terminal control, and filesystem
  watching remain explicit runtime concerns rather than being hidden behind a
  universal I/O service.

This is targeted deepening for safety and testability, not authorization for a
broad TUI rewrite or additional workspace crates.

## Consequences

- Read adapters remain small and independently testable while write safety gains
  a single locality.
- Unsupported operations derive from session capabilities instead of loosely
  coordinated optional fields and booleans.
- Founding claims that `DiffSource` or `margin-vcs` are the only outside-world
  seam are historical descriptions, not current architecture.
- The Elm update/view boundary and existing crate dependency graph remain
  stable through v0.6.
