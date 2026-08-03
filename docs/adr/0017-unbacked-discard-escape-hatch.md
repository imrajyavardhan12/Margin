# ADR-0017: Unbacked discard is an explicit, per-invocation escape hatch

- **Status:** Accepted
- **Date:** 2026-08-02
- **Amends:** ADR-0008 and ADR-0014

## Context

ADR-0014 allows users to disable discard backups persistently with
`discard_trash = false`, and ADR-0008 says every configuration option has a
CLI equivalent. Persistent opt-out weakens Margin's central trust promise: a
forgotten setting can make an ordinary discard unrecoverable. Always retaining
a backup is also inappropriate when a developer is deliberately removing
sensitive material and does not want another copy persisted under the gitdir.

## Decision

Discard remains recoverable by default. An unbacked discard is permitted only
through a deliberately explicit CLI option for the current invocation, named
`--discard-without-backup`; the choice cannot be stored in user or repository
configuration. Typed confirmation must state that recovery will not be
available before issuing an unbacked discard.

The existing `discard_trash = false` setting is deprecated and will be removed
under ADR-0008's configuration deprecation policy. ADR-0008's flag/config
parity rule does not apply to safety-critical, one-invocation escape hatches.
This supersedes only ADR-0014's persistent opt-out and ADR-0008's universal
parity rule; their remaining decisions stand.

## Consequences

- A normal Margin invocation cannot silently inherit unrecoverable discard
  behavior from old configuration.
- Developers can avoid retaining sensitive discarded content, but must make
  that decision explicitly each time.
- Issue #89 must not add the proposed `--no-trash` flag as written.
- The unsafe path needs dedicated CLI, confirmation, and transaction tests.
