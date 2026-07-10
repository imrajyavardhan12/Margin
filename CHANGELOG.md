# Changelog

All notable changes to Margin. Generated from Conventional Commits by git-cliff;
release notes are hand-curated on top in GitHub Releases.

## [Unreleased]

### Added

- Act on the diff (issue #10): `s` stages and `u` unstages the hunk under
  the cursor, applying exactly the reviewed hunk bytes to the git index —
  never the working tree — then reloading and re-anchoring the cursor.
  Stale hunks, binary files, renames, and non-git sources report in the
  status bar instead of failing. In worktree reviews the sidebar marks
  files that have staged content with a dot, so partial staging is visible
  at a glance; the marker refreshes as you stage and unstage.
- Reload: `r` re-reads the diff from its source without leaving the
  review, keeping your place. Works in every mode with a live source
  (worktree, `--staged`, revisions, files); staging feedback points at it
  when a hunk no longer applies.
- Staging feedback tells the truth about the common misfires: staging an
  already-staged hunk says so (instead of "changed since load"), and
  unstaging a file with nothing staged refuses up front.
- Collapse (issue #21): `za` folds the cursor's file to its header
  (counts stay visible), `zA` folds or unfolds everything. Lockfiles
  (`Cargo.lock`, `package-lock.json`, `go.sum`, ...) and generated
  artifacts (`*.min.js`, `*.pb.go`, source maps, ...) fold automatically;
  the `collapse` config key (user or repo — it is a display option)
  adds globs. Navigation skips folded bodies, search never matches
  inside them, and fold choices survive watch-mode reloads.
- Watch mode (issue #12): `margin -w` / `margin diff -w` reloads the
  review automatically while an agent edits — OS file events, debounced
  (rapid writes collapse into one reload), cursor and search kept in
  place, `[watch]` in the status bar. External staging and new commits
  refresh too (the index and reflog are watched; object churn is not).
  Auto-reload never fires while the discard confirmation is open.
  Worktree and `--staged` reviews only; static views refuse the flag.
- Discard (issue #11): `x` removes the hunk under the cursor from the
  working tree — Margin's only destructive action, so it is guarded twice
  (ADR-0014): a prompt that only typed `yes` + Enter confirms, and a
  backup patch written to `.git/margin/trash/` **before** anything is
  applied. `margin undo` restores the most recent discard;
  `discard_trash = false` (user config only) opts out of backups. Stale
  hunks refuse cleanly; the index is never touched, so staged copies
  survive a discard exactly as with `git restore`.

## [0.1.0-rc.1] - 2026-07-03

### Added

- Line wrap: `w` wraps long lines in both unified and split views instead
  of clipping them (issue #14). Wrapped rows scroll as one unit, the cursor
  keeps its full row on screen, wrapping never splits a double-width
  character, and syntax colors, intra-line emphasis, and search highlights
  all carry across continuation rows. The status bar shows `[wrap]`.
- The `?` help overlay now lists search, the file picker, and the layout
  toggle (it had fallen behind the keymap).
- Fuzzing (issue #8): three cargo-fuzz targets — `parse_unified` (full parse →
  display → intraline pipeline with safety-contract asserts), `strip_ansi`
  (never grows input, never leaks ESC, idempotent), and `intraline` (every
  range sliceable on UTF-8 boundaries) — seeded from the patch corpus, run
  weekly in CI with a smoke run on parser PRs.
- Search: `/` opens incremental smart-case regex search over file paths and
  line contents (both sides in split view); matches highlight inline,
  `n`/`N` wrap-navigate, the status bar shows a position badge, and invalid
  regexes report instead of failing. A keystroke over a 250k-line diff
  costs ~10 ms (allocation-free byte scanning).
- Fuzzy file picker: `f` opens a jump-to-file overlay with dependency-free
  subsequence matching that prefers tight, early matches.
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
