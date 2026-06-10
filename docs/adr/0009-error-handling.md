# ADR-0009: Errors: thiserror in libs, anyhow at the edge, no panics

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

Margin consumes untrusted input (arbitrary patches on stdin, arbitrary repo
states) inside a raw-mode terminal. A panic in raw mode doesn't just crash —
it leaves the user's terminal broken. And from v0.2, Margin holds write
capabilities over the user's working tree, where "crashed mid-operation" must
have a defined answer.

## Decision

1. **Library crates (`margin-core`, `margin-vcs`, `margin-tui`) never panic
   on input.** They return typed errors (`thiserror`-derived enums). Workspace
   lints warn on `unwrap`/`expect`; CI runs clippy with `-D warnings`, so
   every use needs an explicit `#[allow]` with a justification comment.
   Slice indexing on untrusted data uses `get()`.
2. **The binary uses `anyhow`** for context-rich error chains at the edge:
   `margin: cannot open repository at ./x: not a git repository` — message
   first, chain below, exit code 2 or 1 per ADR-0007.
3. **A panic hook + terminal guard** restore the terminal (leave raw mode,
   alternate screen) before any abort, then print the panic with an issue-link
   footer. The guard is RAII so early returns restore too.
4. **Recoverable runtime errors are `Msg`s**, rendered as a status-line
   message in the UI (e.g., reload failed because the repo is mid-rebase),
   never a crash, never a silent swallow. Every `Err` is either shown to the
   user or converted to a typed state — discarding errors is a review reject.
5. **Fuzzing enforces rule 1** for the parser (ADR-0010).

## Consequences

- "Never breaks your terminal, never eats an error" becomes testable, not
  aspirational.
- Error messages are a designed surface: libs carry structured causes, the
  edge renders them with context.
- Cost: typed error enums are more work than `anyhow` everywhere — accepted,
  because callers (the TUI) must *distinguish* failures to react correctly.

## Alternatives considered

- **`anyhow` everywhere** — fine at the edge, but erases the type information
  the TUI needs to decide between "show message" and "degrade view".
- **Panic-as-control-flow with `catch_unwind`** — hides logic errors and is
  unacceptable in the same process that applies patches to the index.
