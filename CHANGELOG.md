# Changelog

All notable changes to Margin. Generated from Conventional Commits by git-cliff;
release notes are hand-curated on top in GitHub Releases.

## [0.5.1] - 2026-08-02

### Fixed

- Review notes were invisible while the cursor sat on their hunk — the
  moment right after typing one. They used the dim `meta` style, whose
  grey is the cursor line's background grey in several themes. Notes now
  have their own theme slot (`note`, overridable in custom themes), and
  a style-assertion test checks legibility on the cursor line in every
  theme and colour mode — symbol snapshots cannot see colour, which is
  why this shipped in 0.5.0 unnoticed.

## [0.5.0] - 2026-08-01

The reviewer's half of the agent loop: annotate what you read, export it
for whoever fixes it.

### Added

- Review notes (issue #23): `c` annotates the hunk under the cursor with
  a one-line remark, shown inline on the hunk header with a `✎` count in
  the sidebar. Enter saves, empty input deletes, Esc cancels. Notes
  persist per review beside viewed marks, keyed by a digest of that hunk,
  so they survive reloads and detach when the hunk itself changes.
- `margin --notes` prints those notes as Markdown instead of opening the
  TUI — each note under its file with a `path:line` anchor and its hunk
  header, ready to paste into a pull request or hand to an agent. Works
  for every review source (`--notes` is global, like `--json`, and the
  two refuse to be combined).

### Performance

- The sidebar's staged-file summary no longer loads a full index-vs-HEAD
  changeset (issue #62). It enumerates diff deltas instead, so the cost
  tracks the number of staged files rather than their content — measured
  at 101 ms → 2.2 ms (45x) on a repo with 800 staged files, on every
  stage, unstage, discard, and reload.

## [0.4.0] - 2026-08-01

Make it yours: custom themes, mouse support, shell completions, and
runnable examples.

### Added

- Mouse support (issue #26): the wheel scrolls, a left click places the
  cursor on the clicked row, a click in the sidebar jumps to that file,
  and a click closes the help overlay. Strictly additive — the keyboard
  stays primary, overlays keep their keyboard grammar, and
  `mouse = false` in config (or `--no-mouse`) opts out entirely,
  keeping the terminal's own text selection.

- Custom themes (issue #15): a `[themes.<name>]` section in the user
  config inherits a built-in `base` and overrides individual `#rrggbb`
  colors and/or the `syntax_theme`; `theme = "<name>"` selects it, and
  naming one after a built-in tweaks what `auto` picks. Unknown keys
  and malformed colors are config errors naming the key. Repo-local
  `.margin.toml` cannot define themes (a hostile repo could restyle
  additions into invisibility). Degraded modes (16-color, `NO_COLOR`)
  still apply unchanged. Schema documented in docs/themes.md.

- `margin completions <bash|zsh|fish|powershell>` prints shell
  completions, and the hidden `margin man` prints the roff man page
  (issue #16, ADR-0016) — generated at runtime from the installed
  binary, so they can never drift from your version. Both work even
  with a broken config file, so an `eval` in a shell rc is safe.

- An `examples/` directory of runnable demo patches (issue #17):
  syntax + intraline, rename + mode change, binary files, format-patch
  mail, unicode paths — try Margin with no repository at hand. CI
  asserts every example opens with zero parse warnings.

## [0.3.0] - 2026-07-30

Pull-request review, review persistence, and a terminal-aware default
theme.

### Security

- `margin pr` rejects flag-shaped selectors before they ever reach the
  authenticated `gh` CLI's argv (argument injection), and both `gh`
  invocations pass the selector after `--` as defense in depth.

### Added

- The default theme is now `auto` (issue #27): Margin queries the
  terminal's background color (OSC 11, 50 ms budget) and picks `ledger`
  on dark, `foolscap` on light. Unanswering terminals (tmux/screen
  without passthrough, CI, pipes) fall back to `ledger` — the previous
  default — and an explicit theme anywhere skips the query.

- `--no-untracked` excludes untracked files from worktree reviews
  (issue #18) — the flag form of the `include_untracked` config key,
  completing flag/config parity (ADR-0008). `--dump-config` reflects it.

- `margin pr <number|branch|url>` reviews a GitHub pull request through
  the authenticated `gh` CLI (issue #24, ADR-0015) — Margin never holds
  a token. Viewed marks persist per PR and survive force-pushes for
  untouched files (content digests do the invalidation). `--json` works
  here too. Clear errors when `gh` is missing or not logged in.
- The status bar shows `hunk x/y` for the hunk under the cursor
  (issue #19) — reviewers think in hunks, not rows.
- Mark viewed (issue #20): `m` checks off the cursor's file — sidebar
  checkmark, body folded (`za` reopens without unmarking). Marks persist
  per review under the data dir, keyed by a content digest: quitting and
  relaunching the same diff keeps your place, a rebase keeps untouched
  files marked, and any file that changed un-views itself. Patch and
  pager reviews keep marks session-only; nothing is written for them.

## [0.2.0] - 2026-07-11

Structured output for scripts and agents, plus hardening from the
post-M2 review sweep.

### Fixed

- Watch mode no longer reloads while the fuzzy picker is open (the world
  shifting under a half-made choice could jump to the wrong file), and a
  reload that does happen refilters an open picker against the new
  changeset instead of leaving stale indices.
- Watch mode's debounce now has a maximum wait: a sustained write storm
  (an agent writing continuously) can no longer starve the reload — after
  ~8 quiet-windows of continuous activity the review refreshes anyway.
- Discard backups are written with an atomic create-new, closing a window
  where two same-millisecond discards from different margin instances
  could silently overwrite each other's trash entry.
- Watch mode no longer re-renders an unchanged frame ten times a second
  while idle.

### Added

- JSON output (issue #22): `--json` on `diff`, `show`, and `patch` emits
  the parsed changeset as a versioned document (`"schema": 1`) — files,
  hunks, and lines with statuses, renames, binary flags, modes, and
  counts. Strings are lossy UTF-8 with honesty flags on anything that
  had invalid bytes. Schema documented in `docs/json-output.md`; within
  schema 1 changes are additive only. Pager passthrough is untouched;
  `--watch` and `--json` refuse to combine.

## [0.1.0] - 2026-07-12

The first stable release. Everything from the release candidate, plus the
"act on the diff" feature set: stage, unstage, discard (with undo), reload,
watch mode, and collapse.

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
