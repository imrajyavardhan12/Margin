# ADR-0024: SHA-pinned, least-privilege CI supply chain

- **Status:** Accepted
- **Date:** 2026-08-02

## Context

Margin's GitHub Actions build, test, publish documentation, update the Homebrew
formula, and produce release binaries. Mutable action tags make those trusted
jobs depend on code that can change without a repository commit, including jobs
with write permissions and artifact authority.

## Decision

Every reusable GitHub Action, including GitHub-owned actions, is pinned to a
full commit SHA with a version comment for readability. Dependabot maintains
those pins. Every workflow declares the minimum permissions it needs, and the
repository restricts execution to an approved action set.

After existing workflows are migrated, repository policy enforces full-SHA
pinning. Generated cargo-dist workflow changes receive the same review and must
be repinned before merge; generated ownership does not exempt release code from
supply-chain policy. Dependabot security updates remain enabled alongside the
scheduled Rust advisory check.

## Consequences

- Workflow code changes become visible, reviewable repository changes.
- Release credentials and artifacts have a smaller compromise surface.
- Action updates require routine maintenance, and cargo-dist regeneration may
  require a pin-refresh step before CI accepts the result.
