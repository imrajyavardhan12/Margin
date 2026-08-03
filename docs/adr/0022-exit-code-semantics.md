# ADR-0022: Distinguish clean, operational, and invocation outcomes

- **Status:** Accepted
- **Date:** 2026-08-02
- **Supersedes:** ADR-0007's ambiguous `displayed-with-errors` exit-code wording

## Context

ADR-0007 reserves exit codes `0`, `1`, and `2`, but the Implementation never
returns `1`; repository, I/O, undo, and source failures currently return `2`
alongside malformed command usage and configuration. Scripts therefore cannot
rely on the documented distinction, and a usable partial parse cannot signal
that warnings occurred.

## Decision

Margin's stable exit-code contract is:

- `0`: the requested operation completed cleanly;
- `1`: an operational failure occurred, or usable output was produced with one
  or more warnings;
- `2`: the invocation or configuration is invalid.

Pager passthrough remains byte-identical and exits `0`, including an ordinary
broken pipe caused by a downstream command finishing early. Patch or JSON
output that is usable but has parser warnings is emitted and exits `1`.
Interactive review exits `1` when the session showed an operational write or
persistence failure, even if the developer continued reviewing before quit.

## Consequences

- Existing successful and usage-error values do not change; the previously
  unused `1` gains precise semantics.
- Runtime errors currently returning `2` must be reclassified and covered by
  binary integration tests.
- The TUI runtime must return a session outcome rather than only terminal I/O
  success, without making non-fatal errors end the review.
