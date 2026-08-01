# ADR-0016: Completions and the man page are generated at runtime

- **Status:** Accepted
- **Date:** 2026-08-01
- **Supersedes / Superseded by:** —

## Context

Issue #16 wants shell completions (bash/zsh/fish/powershell) and a man
page from the existing clap definitions. clap offers two delivery
routes: generate files at build time (a `build.rs` running
`clap_complete`/`clap_mangen`, artifacts wired into packaging) or
generate on demand from the installed binary (a subcommand).

## Decision

Generation happens **at runtime, from the installed binary**:

1. `margin completions <shell>` prints the completion script to stdout —
   a visible verb, because users run it themselves
   (`margin completions zsh > "$fpath[1]/_margin"` or an `eval` in a
   shell rc file).
2. `margin man` prints the roff page to stdout — **hidden**, because its
   audience is packagers and the curious (`margin man | man -l -`);
   interactive users read `--help`. Hidden still means stable: it is a
   CLI verb and carries the same compatibility promise as the rest
   (ADR-0007).
3. Both dispatch **before config loading**. Completions are eval'd from
   shell startup files; a typo in `config.toml` must never break a
   shell. This ordering is load-bearing and tested.
4. Output goes through a buffer, and a broken pipe on stdout is success,
   not an error — `margin completions zsh | head` is normal use, and
   `clap_complete::generate` panics if handed a failing writer directly.

Why not build time:

- **Drift-proof.** The script always matches the binary that generates
  it. Pre-built files go stale against whatever version a user actually
  has installed — the classic completions bug.
- **cargo-dist cannot run what it builds.** Release artifacts are
  cross-compiled; the build host cannot execute an aarch64-linux binary
  to harvest its completions. Every packaging route that wants files
  (Homebrew's `generate_completions_from_executable`, distro packages)
  runs the *target* binary at install time — which is exactly the
  interface this ADR ships.
- **Hermetic builds stay simple.** No `build.rs`, no generated files in
  the archive layout, no `dist generate` churn.

## Consequences

- Easier: one code path, zero packaging plumbing, `--help`/man/completions
  can never disagree about the CLI surface.
- Harder: completions are not pre-installed by the shell archives; users
  (or formulas) run one command once. The README documents each shell's
  one-liner.
- Committed to: new verbs and flags appear in completions and the man
  page automatically — no release checklist item exists to forget.

## Alternatives considered

- **`build.rs` + cargo-dist artifact wiring**: broken by design for
  cross-compiled targets (see above); adds generated-file drift and
  build complexity for zero user benefit. Rejected.
- **A `margin generate` umbrella verb** (completions + man under one
  namespace): two flat verbs are simpler, and `completions` matches the
  de-facto convention users already know from other tools. Rejected.
