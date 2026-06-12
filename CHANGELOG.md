# Changelog

All notable changes to Margin. Generated from Conventional Commits by git-cliff;
release notes are hand-curated on top in GitHub Releases.

## [Unreleased]

### Added

- Configuration (ADR-0008): user `config.toml` (XDG paths, `$MARGIN_CONFIG`
  override), repo-local `.margin.toml` restricted by schema to display
  options, `--theme`/`--layout` flags, and `margin --dump-config`. Unknown
  keys error with did-you-mean suggestions.
- Four built-in themes — `ledger` (default dark), `foolscap` (light),
  `carbon` (high contrast), `blueprint` (blue dark) — each with a matched
  syntax palette, plus deliberate degradation: one ANSI-16-safe palette on
  non-truecolor terminals (syntax off) and a `NO_COLOR` monochrome mode
  using bold/dim/reverse only.
- The full git-verb CLI (clap): `margin diff [--staged] [<rev>|A..B|fileA fileB]`,
  `margin show [rev]`, `margin patch [-|file]`, and `margin pager`.
  `margin diff <rev>` diffs the working tree against that revision.
- The pager passthrough guarantee: `pager`/`patch` modes with piped stdout
  write input through byte-identical and exit 0 — safe to set as
  `git config core.pager` permanently (integration-tested against colored
  `git log -p`, invalid UTF-8, and missing trailing newlines).
- ANSI stripping in margin-core: git colorizes output sent to pagers;
  Margin parses it cleanly and still passes raw bytes through untouched.
- Two-file diffs without a repository (git2 buffer diffing, binary-aware).

## [0.1.0-alpha.2] - 2026-06-12

### Added

- Syntax highlighting (syntect, ~200 languages by extension) layered under
  addition/deletion background tints, plus word-level intra-line emphasis
  (`similar`) on paired changed lines — with a rewrite heuristic that keeps
  emphasis off mostly-changed lines.
- Lazy, budgeted rendering: at most a few hundred lines are highlighted per
  frame app-wide; oversized hunks fill in across frames while the input
  loop stays responsive. Measured first paint (release): ~4 ms on a
  100-file/10k-line diff, ~15 ms on a 250k-line single-hunk file.
- Criterion benchmarks for parsing and frame times, wired into CI as an
  informational job on main.

## [0.1.0-alpha.1] - 2026-06-11

First installable pre-release: the read-only viewer core. Expect rough
edges; syntax highlighting (#4), stdin/pager modes (#5), themes (#6), and
search (#7) are still in flight on the road to v0.1.0.

### Added

- Project foundation: four-crate workspace, ADRs 0001–0012, CI, governance
  docs, issue/PR templates.
- `margin-core`: changeset data model (bytes-first) and a tolerant
  unified-diff parser covering git extended headers, renames, binary files,
  mode changes, C-quoted paths, no-newline markers, plain `diff -u` output,
  and `git log -p` streams, with a corpus regression suite.
- `margin-vcs`: git2-backed sources — worktree vs HEAD (untracked files
  included by default), staged, `show` (incl. root commits), and revision
  ranges — with rename/copy detection, typed errors, and temp-repo
  integration tests. The binary prints a changeset summary as a walking
  skeleton until the TUI lands.
- `margin-tui`: the interactive review UI — file sidebar with statuses and
  counts, unified diff pane with dual line numbers and hunk headings,
  vim-grammar navigation (`j/k`, `J/K`, `]/[`, `gg/G`, `Ctrl-d/u`), help
  overlay, responsive sidebar, control-character sanitization, and a panic
  guard that always restores the terminal. The binary launches the TUI on a
  terminal and prints a plain summary when piped.
- Side-by-side layout: deletions and additions paired on aligned rows with
  per-side line numbers, unicode-width-aware fitting, and a width-based
  `auto` mode (split at 120+ columns); `v` pins unified or split, and the
  cursor keeps its place when layouts switch.
