# Changelog

All notable changes to Margin. Generated from Conventional Commits by git-cliff;
release notes are hand-curated on top in GitHub Releases.

## [Unreleased]

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
