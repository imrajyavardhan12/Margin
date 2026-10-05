# ADR-0027: Refuse invocations that would review something other than what was given

- **Status:** Proposed
- **Date:** 2026-10-05
- **Supersedes / Superseded by:** — (applies ADR-0007's "refuse loudly" rule to two cases it did not name)

## Context

Margin's worst failure is showing a reviewer a different changeset than the
one they meant to review. They then accept or annotate the wrong code while
believing they checked the right code. Two invocations did exactly that,
silently:

- `git diff | margin` (and `margin < file`): a bare `margin` reviews the
  working tree and never reads stdin, so the piped diff was ignored
  (issue #119). For `git diff` the two often coincide. For
  `git show | margin` or `git diff main | margin` they do not.
- Root flags placed before a subcommand: `margin --staged diff` reviewed the
  working tree, `margin -w show` dropped `-w`, and `margin --json undo`
  ran a real working-tree write instead of producing a document.

Stdin is a hard signal. Non-interactive callers hand over a stdin pipe
that means nothing: scripts, git hooks, `ssh host margin`, `while read`
loops, and agent tools whose subprocesses default to `stdin=PIPE`. Those
callers mostly want documents (`--json`, `--notes`) or a piped summary.
Margin's primary users drive it from coding agents (ADR-0023), so breaking
`margin --json` under a pipe would break the core audience. Reading stdin
"just in case" would swallow a script's loop input, and blocks forever on
a pipe whose writer never closes.

## Decision

Margin refuses (exit 2, ADR-0022) an invocation whose input or flags it
would otherwise ignore while reviewing something else. It never guesses a
different mode.

- A bare `margin` refuses redirected stdin (a pipe, socket, or regular
  file; not a terminal or `/dev/null`) only when it would open the
  interactive review: stdout is a terminal and neither `--json` nor
  `--notes` was given. The message names both explicit forms
  (`... | margin patch`, and `margin diff` or `margin diff --staged`).
  Stdin is classified by file type and never read. The check runs before
  any terminal query, such as auto-theme detection.
- Root `--staged` and `-w` apply to `diff` when the subcommand is `diff`.
  With `show`, `patch`, `pr`, or `undo`, they refuse. Root `--json` and
  `--notes` refuse with `undo`.
- `margin pager` is exempt from all of the above. Its byte-identical
  passthrough contract (ADR-0007) outranks any flag.
- Detection is Unix-only for now. On Windows, file attributes alone cannot
  tell a redirected file from the `NUL` device, and a false positive would
  refuse every script run, so Windows keeps the previous behavior until it
  can classify the handle (e.g. `GetFileType`).

## Consequences

- No successful invocation changes meaning: each refusal replaces a review
  of the wrong changeset. Scripts that depended on the old behavior get a
  loud, actionable exit 2 rather than different output.
- Error-first leaves the door open. If users want `git diff | margin` to
  just work, a later ADR can turn the refusal into patch mode without
  breaking anyone. A silent behavior could not be taken back that way.
- Document modes still ignore a redirected stdin silently:
  `git diff | margin --json` documents the working tree. That is the price
  of not breaking non-interactive callers; `margin patch --json` is the
  explicit form.
- Windows users keep the silent `git diff | margin` behavior until the
  platform gap closes.

## Alternatives considered

- **Adopt patch mode on redirected stdin.** It guesses intent, reads input
  scripts never meant to give, and hangs on open pipes. It also cannot be
  reversed later without breaking whoever came to rely on it.
- **Warn on stderr and continue.** In the interactive case, the warning is
  hidden behind the alternate screen until after the reviewer has already
  reviewed the wrong changeset.
- **Refuse in every mode.** It breaks `margin --json` for agents, hooks, and
  `ssh`, the callers Margin most needs to serve.
- **Make root flags conflict with all subcommands (clap
  `args_conflicts_with_subcommands`).** It breaks the documented, working
  `margin --json show` form.
