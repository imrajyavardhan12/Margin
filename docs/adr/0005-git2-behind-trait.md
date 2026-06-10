# ADR-0005: Git access via git2, quarantined behind `DiffSource`

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

Margin needs: working-tree and staged diffs (including untracked files),
revision-range diffs, `show`, rename detection — and in v0.2, *writing*:
staging/unstaging hunks to the index and discarding worktree hunks. Options
are libgit2 bindings (git2), pure-Rust gitoxide (gix), or shelling out to the
`git` binary.

The write path decides it: hunk staging needs patch application to the index.
git2 provides `Repository::apply(ApplyLocation::Index)` — proven in production
by gitui for exactly this feature. gix does not yet cover this surface;
shelling out means generating reverse/partial patches and driving `git apply
--cached --unidiff-zero` with its edge cases as our API.

## Decision

Use **git2** for all repository access — reads now, writes in v0.2 — confined
to `margin-vcs` behind the `DiffSource` trait (and a future `ApplyTarget`
trait for writes). No git2 types in any public signature of `margin-vcs`;
everything crossing the boundary is translated to `margin-core` types.

`default-features = false` on git2: no network transports — Margin never
fetches; it reads local state only. This shrinks the binary and the supply-
chain surface.

## Consequences

- Hunk-level staging in v0.2 has a proven implementation path.
- Diff semantics (renames, mode changes, untracked) come from libgit2's
  battle-tested implementation rather than our reimplementation.
- The trait quarantine means migrating to gix when it matures — or adding jj
  via its CLI — touches one module each (verified by the synthetic-source tests).
- Cost: a C dependency (libgit2, vendored + static). Build times grow;
  the binary stays static. Acceptable.
- Cost: libgit2 occasionally diverges from git's behavior in corners; the
  integration suite (ADR-0010) compares against real `git diff` output on
  generated repos to catch this.

## Alternatives considered

- **gitoxide (gix)** — pure Rust, impressive performance, but index-apply for
  staging isn't there yet. Re-evaluate post-1.0 with a superseding ADR.
- **Shelling out to `git`** — maximum semantic fidelity and zero C deps, but
  fragile output parsing, version skew across user machines, and an awkward
  hunk-staging path. We *do* keep it in tests as the semantic oracle.
- **git2 with network features** — unnecessary attack/bloat surface for a
  tool that never touches a remote.
