# Architecture Decision Records

Significant technical decisions in Margin are recorded here, with the context
and alternatives at the time they were made. The code says *what*; ADRs say
*why* — so future contributors (and future us) don't relitigate settled
questions or, worse, silently unsettle them.

## Process

1. A decision is **significant** if reversing it later would cost more than a
   day, or if a reviewer could reasonably ask "why is it done this way?"
   Dependency choices, crate boundaries, file formats, user-facing contracts,
   and policies (errors, MSRV, releases) all qualify. Naming a variable does not.
2. Copy [`template.md`](template.md) to `NNNN-short-slug.md` (next number),
   open a PR. The PR discussion *is* the decision review.
3. Statuses: `Proposed` → `Accepted` → (`Deprecated` | `Superseded by ADR-NNNN`).
   **Never edit an accepted ADR's decision** — write a new one that supersedes
   it. History is the point.
4. PRs that contradict an accepted ADR must include the superseding ADR.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust.md) | Implementation language: Rust | Accepted |
| [0002](0002-ratatui-crossterm.md) | TUI stack: ratatui + crossterm | Accepted |
| [0003](0003-elm-architecture.md) | App pattern: Elm architecture with a pure core | Accepted |
| [0004](0004-workspace-crates.md) | Four-crate workspace with one I/O seam | Accepted |
| [0005](0005-git2-behind-trait.md) | Git access via git2, quarantined behind `DiffSource` | Accepted |
| [0006](0006-syntect-similar.md) | Highlighting via syntect; intra-line diff via similar | Accepted |
| [0007](0007-cli-design.md) | CLI mirrors Git verbs; pager passthrough guarantee | Accepted |
| [0008](0008-config-toml.md) | Configuration: TOML, XDG + repo-local | Accepted |
| [0009](0009-error-handling.md) | Errors: thiserror in libs, anyhow at the edge, no panics | Accepted |
| [0010](0010-testing-strategy.md) | Testing: snapshots, temp repos, corpus, fuzz, benches | Accepted |
| [0011](0011-release-distribution.md) | Releases: cargo-dist, Conventional Commits, MSRV policy | Accepted |
| [0012](0012-license.md) | License: MIT OR Apache-2.0 | Accepted |
| [0013](0013-hunk-staging-safety.md) | Hunk staging: index-only, dry-run first, exact bytes | Accepted |
| [0014](0014-discard-safety.md) | Discard: trash before destroy, worktree-only, typed confirm | Accepted |
| [0015](0015-forge-via-gh.md) | Forge access through the user's `gh` CLI, quarantined in margin-vcs | Accepted |
