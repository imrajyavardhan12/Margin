# Changelog

All notable changes to Margin. Generated from Conventional Commits by git-cliff;
release notes are hand-curated on top in GitHub Releases.

## [Unreleased]

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
