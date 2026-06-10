# ADR-0007: CLI mirrors Git verbs; pager passthrough guarantee

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

A diff viewer lives inside existing muscle memory. Every new concept in the
CLI is adoption friction; every deviation from git semantics is a bug report.
Hunk validated this: its `diff`/`show`/`patch`/`pager` verb set is one of the
things it got right. Separately, the single most dangerous integration point
is `git config core.pager` — if we misbehave when output is piped, we corrupt
scripts the user didn't even know invoked a pager.

## Decision

The CLI mirrors Git's verbs and defers revision semantics to git:

```
margin                      # = margin diff
margin diff [--staged] [<rev>|<a>..<b>] [-- <paths>]
margin diff <fileA> <fileB> # two arbitrary files
margin show [<rev>]
margin patch [-|<file>]     # unified diff from stdin or a file
margin pager                # core.pager mode
```

Contracts:

1. **Passthrough guarantee:** in `pager` mode (and `patch -`), if stdout is
   not a TTY, Margin writes its input through **byte-identical** and exits 0.
   Setting `core.pager = "margin pager"` must never break `git log -p | grep`.
   This is tested in CI as a release blocker.
2. **No revision parsing of our own:** `<rev>` arguments are resolved by the
   VCS layer with git's own semantics (revspec strings handed to git2).
3. **Exit codes are an API:** 0 success, 1 displayed-with-errors, 2 usage.
   Stable from v0.1 so scripts and agents can rely on them.
4. New verbs require a new ADR; flags within existing verbs do not.

## Consequences

- Zero learning curve for git users; documentation writes itself by analogy.
- `core.pager` integration is safe to recommend in the README from day one.
- The future `--json` flag (BLUEPRINT §3) slots in as an output mode on
  existing verbs, not a new surface.
- Cost: git-semantics delegation means odd revspecs behave exactly as oddly
  as git — we own rendering, not resolution. That's the right ownership line.

## Alternatives considered

- **Own subcommand vocabulary** (`margin review`, `margin open`) — reads
  nicely in a README, fails the muscle-memory test, doubles docs burden.
- **Flags-only, no subcommands** (delta-style) — fine for a pipe prettifier,
  too cramped for an interactive tool with multiple sources.
