# ADR-0026: Highlight warming is per-frame budgeted fill-in, not a background thread

- **Status:** Accepted
- **Date:** 2026-09-12
- **Supersedes:** the "warmed by a background thread" clause of ADR-0006

## Context

ADR-0006 promises that syntax highlighting and intra-line emphasis run
"lazily … cached per file, warmed by a background thread". The
implementation never had that thread: `HighlightCache` works on a
per-frame line budget (`FRAME_BUDGET`), set at the top of every `view`,
with unfinished work resuming across fill-in frames. The budgets the
decision cares about — first paint and scrolling on giant diffs — are met
by bounding work per frame, and the budget mechanics are covered by
deterministic tests rather than by warming.

## Decision

The background-thread warming clause is retracted. The mechanism is:

- highlight and emphasize only lines the current frame requests;
- stop at the per-frame budget and report pending work;
- resume across subsequent frames until complete.

No new thread, no shared cache invalidation, no shutdown protocol — the
simplest mechanism that meets the budgets. The laziness and caching
clauses of ADR-0006 stand unchanged.

## Consequences

- Architecture documentation must describe budgeted fill-in, not warming.
- Performance enforcement (issue #99) rests on the deterministic budget
  test plus workload completion tests, not on warming behavior that was
  never built.
