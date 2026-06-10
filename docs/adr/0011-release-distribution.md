# ADR-0011: Releases: cargo-dist, Conventional Commits, MSRV policy

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

Install friction is our competitive wedge against Hunk (npm/Node). That
promise dies if releases are hand-rolled: missing targets, broken checksums,
stale Homebrew formulas. Release engineering must be boring and automated
from the first tag.

## Decision

- **cargo-dist** owns release builds. Targets from v0.1:
  `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` (static),
  `x86_64-pc-windows-msvc`.
  Artifacts: tarballs/zips + sha256 checksums, `curl | sh` and PowerShell
  installers, Homebrew formula pushed to `imrajyavardhan12/homebrew-tap`, cargo-binstall
  metadata.
- **Versioning:** SemVer. Pre-1.0, minor = features, patch = fixes; breaking
  config/keybinding changes get a deprecation cycle of ≥2 minors (ADR-0008).
  All workspace crates version in lockstep and publish to crates.io.
- **Commits:** Conventional Commits, enforced on PR titles (squash-merge
  strategy, so PR title = commit message). **git-cliff** generates
  CHANGELOG.md; release notes are hand-curated on top.
- **MSRV:** current stable minus two releases, recorded as `rust-version` in
  the workspace, tested in CI. Raising MSRV is a `feat!`-level note in the
  changelog, never in a patch release.
- **Cadence:** tag when ready pre-1.0, aiming for a minor every 4–6 weeks.
  A release tag requires: CI green, benches within budget, CHANGELOG updated,
  install verified from artifacts on the three OSes (checklist in RELEASING
  section of CONTRIBUTING).
- Post-1.0 channels (AUR, nixpkgs, winget, scoop, mise) are community-driven;
  we keep cargo-dist outputs stable so packagers have an easy time.

## Consequences

- `brew install`, `curl | sh`, `cargo binstall`, and direct download all work
  from the first public tag — the README's install section is honest on day one.
- Squash + Conventional Commits keeps history linear and the changelog free.
- Cost: cargo-dist config is generated and occasionally needs regeneration on
  upgrades; we pin its version and upgrade deliberately.
- Cost: musl static builds constrain us to pure-Rust or vendored-C deps —
  already satisfied by ADR-0005 (vendored libgit2) and ADR-0006 (fancy-regex).

## Alternatives considered

- **Hand-written release workflows** — every project that starts here rewrites
  to cargo-dist or goreleaser-style tooling after the third broken release.
- **npm distribution as an extra channel** (Hunk's model) — wrong signal for
  a no-runtime tool; binstall + brew + curl cover the same users better.
- **CalVer** — SemVer's compatibility signal matters for a tool with config
  and keybinding stability promises.
