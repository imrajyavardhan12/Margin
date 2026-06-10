# ADR-0002: TUI stack: ratatui + crossterm

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

We need a terminal UI layer that (a) renders efficiently enough for
virtualized scrolling over huge diffs, (b) works on macOS, Linux, and Windows
Terminal, (c) is testable headlessly in CI, and (d) is familiar to the
contributors we want to attract.

## Decision

Use **ratatui** (immediate-mode TUI) with the **crossterm** backend.
Terminal capability handling (raw mode, alternate screen, resize, color
depth) goes through crossterm only — no direct escape-sequence writing
outside `margin-tui`.

## Consequences

- Immediate-mode rendering fits the Elm architecture (ADR-0003): `view()` is a
  pure function of state, re-rendered per frame, which makes frame snapshots
  with `TestBackend` + insta the backbone of UI testing (ADR-0010).
- crossterm gives us the three OS targets including Windows, which delta-class
  tools often neglect — part of our "terminal citizenship" pitch.
- ratatui is the de-facto standard: largest contributor familiarity, active
  maintenance, a showcase channel that doubles as a launch venue.
- Cost: immediate mode means we own scroll virtualization and damage
  avoidance ourselves; the < 50 ms first-paint and 60 fps budgets are *our*
  job, enforced by benchmarks (ADR-0010).
- Cost: no built-in widget retained state; all state lives in `AppState`,
  which is exactly what ADR-0003 wants anyway.

## Alternatives considered

- **Cursive** (retained mode) — widget-owned state fights the Elm pattern and
  headless testing; smaller community.
- **Notcurses bindings** — powerful, but C dependency breaks the
  pure-static-binary story and Windows support is weak.
- **Custom renderer on raw crossterm** — maximum control, but we would
  reimplement layout/buffer-diffing that ratatui already does well; revisit
  only if profiling proves ratatui is the bottleneck (write a superseding ADR).
