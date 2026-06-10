# ADR-0003: App pattern: Elm architecture with a pure core

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

TUI apps rot in a specific way: input handling, state mutation, and rendering
interleave until no interaction can be tested without a live terminal. We are
also about to add *destructive* operations in v0.2 (discard hunks), where "I'm
not sure which state the keypress fired in" becomes data loss.

## Decision

`margin-tui` follows the **Elm architecture (TEA)**:

- One `AppState` struct — the single source of truth.
- One `Msg` enum — every interaction and async result is a message.
- `update(&mut AppState, Msg) -> Vec<Command>` — the only place state changes.
  Side effects (load source, write to index) are returned as `Command` values
  and executed by the runtime shell, never performed inside `update`.
- `view(&AppState, &mut Frame)` — pure rendering, no side effects, no state.

Supporting rule: **`margin-core` is pure** — no I/O, no TUI types, panic-free
on untrusted input. All effectful code lives in `margin-vcs` (ADR-0004) or the
binary's runtime shell.

## Consequences

- Every interaction is testable as plain function calls: feed `Msg`s, assert
  state, snapshot the frame. CI covers keybinding flows without a TTY.
- Destructive operations become auditable: exactly one `Msg` can trigger a
  discard, and it's gated by a confirmation state machine visible in one place.
- Custom keymaps later are a data change (key → `Msg` table), not a refactor.
- Cost: boilerplate — every new interaction touches `Msg`, `update`, and
  possibly `Command`. We accept this; it's the price of the audit trail.
- Cost: discipline is needed to keep `update` from doing I/O "just this once".
  Code review enforces; clippy's `disallowed_methods` can enforce mechanically
  later if violations appear.

## Alternatives considered

- **Ad-hoc widget state (typical ratatui example style)** — fastest to start,
  unmaintainable at the feature count we're planning; untestable interactions.
- **Full async actor model** — overkill for one window and one data source;
  TEA's `Command` list gives us async loading without the machinery.
