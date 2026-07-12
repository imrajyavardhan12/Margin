# ADR-0015: Forge access through the user's `gh` CLI

- **Status:** Accepted
- **Date:** 2026-07-12
- **Supersedes / Superseded by:** — (complements ADR-0005)

## Context

Issue #24 wants `margin pr 123`: review a GitHub pull request without
leaving the terminal. That requires talking to a forge — authentication,
API endpoints, rate limits, enterprise hosts — none of which Margin
handles today, and all of which are somebody's full-time job to get
right. Meanwhile ADR-0005 keeps git access in-process (vendored
libgit2, no subprocess) for hermetic builds and byte-level control.

## Decision

Forge access goes through the **user's own `gh` CLI as a subprocess**;
Margin never holds a token, opens a network connection, or implements
an API client.

1. `gh` already solves auth (keyring, SSO, enterprise hosts), pagination,
   and rate limits, and the user has already trusted it with their
   credentials. Margin inherits all of that by exec'ing it.
2. The subprocess boundary lives in **margin-vcs** (`gh.rs`), the crate
   that owns every way a changeset enters Margin. `GhPr` is just another
   `DiffSource`: `gh pr view` once for the canonical identity, `gh pr
   diff` for the bytes, parsed by the same tolerant parser as stdin
   patches. Nothing anywhere else knows a subprocess exists.
3. This does **not** loosen ADR-0005: git object access stays in-process
   via vendored libgit2. The distinction is principled — git repositories
   are local data Margin owns reading; forges are remote authenticated
   services Margin refuses to hold credentials for.
4. Viewed-state identity is `(host, repo, PR number)` — *not* the head
   SHA the issue sketched: per-file content digests (ADR of record:
   issue #20's design) already invalidate precisely, so untouched files
   stay checked across force-pushes, like the rebase case.
5. Errors are the user's `gh` speaking: a missing binary yields
   "margin pr needs the GitHub CLI", a nonzero exit passes `gh`'s own
   stderr through (auth prompts, 404s, disabled PRs) — Margin adds no
   second vocabulary of forge errors.

## Consequences

- Easier: enterprise GitHub, tokens, SSO, 2FA — all inherited, zero code.
- Easier: `gh`'s stderr is the error UX; users already know it.
- Harder: `gh` becomes a runtime (not build) dependency for this one
  verb; everything else works without it. CI tests use a fake `gh` on
  PATH, so the suite stays hermetic.
- Committed to: other forges arrive the same way (`glab` for GitLab,
  `jj`'s tooling for #25) rather than as API clients.

## Alternatives considered

- **octocrab / a GitHub API client**: tokens in Margin's config, a
  second auth story, enterprise-host handling, rate-limit code — a
  liability surface for zero user benefit over `gh`. Rejected.
- **`git fetch` of `refs/pull/N/head` + local diff**: keeps everything
  in libgit2, but requires fetch rights/refspec knowledge, misses PR
  metadata, and silently diverges from what GitHub shows. Rejected.
- **Bundling a token prompt**: Margin holding credentials is exactly
  what this ADR exists to refuse.
