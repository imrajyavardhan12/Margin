# ADR-0021: Selective user-facing compatibility before 1.0

- **Status:** Accepted
- **Date:** 2026-08-02

## Context

Margin is pre-1.0 and still validating its architecture, but users and agents
already depend on command behavior, configuration, structured JSON, install
paths, keybindings, and persisted review state. Treating every minor release as
permission to break those surfaces would undermine adoption; freezing private
Rust APIs now would preserve the least valuable constraints.

## Decision

The following user-facing surfaces are stable before 1.0:

- CLI verbs, flag meanings, install URLs, and documented exit-code semantics;
- configuration keys, with at least two minor releases of deprecation before
  removal;
- each published JSON schema version, which may change additively only;
- default keybindings, with documented migration when a change is unavoidable;
- persisted review state, with automatic forward migration.

Additive changes remain allowed. An intentional incompatibility requires a
migration path, prominent release notes, and a superseding decision when it
contradicts an ADR.

Internal Rust APIs, private Module layout, tests and snapshots not exposed as a
documented format, and other Implementation details remain unstable until 1.0.
Margin's workspace crates are not published as a supported library API.

## Consequences

- Developers can build habits and scripts around Margin during product
  validation without freezing its internals.
- Compatibility tests are required for stable surfaces, including old review
  state fixtures and versioned JSON fixtures.
- Release review must classify changes by user-facing impact rather than relying
  only on Cargo semver.
