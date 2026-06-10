# ADR-0006: Highlighting via syntect; intra-line diff via similar

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

Two rendering-quality features define how the diff *looks*: syntax
highlighting and intra-line (word-level) change emphasis. Both have a
performance dimension — they are the expensive part of rendering and must
never block scrolling (quality bar: 250k-line diffs at 60 fps).

The tempting maximalist choice is tree-sitter: real parse trees, and a path
to structural diffing. But grammars must be bundled per language (binary
bloat), C compilation per grammar, and structural diffing is a product we
explicitly deferred (difftastic owns it; BLUEPRINT §3).

## Decision

- **syntect** (Sublime-grammar engine, as used by bat and delta) for syntax
  highlighting, with precompiled binary assets, `default-features = false`
  + `default-fancy` (pure-Rust regex engine — keeps the no-C-toolchain build
  for contributors and musl targets trivial).
- **similar** for intra-line diffing: word-granularity inline spans computed
  per hunk-line-pair, unicode-aware.
- Both run **lazily**: highlight/intra-line only what scrolls into view,
  cached per file, warmed by a background thread. The input loop never waits.

## Consequences

- Same visual quality users already trust from bat/delta, ~200 languages free.
- Pure-Rust build keeps `cargo install margin` working everywhere without
  oniguruma headaches.
- Laziness makes the giant-diff budgets achievable; the criterion benchmarks
  (ADR-0010) hold the line.
- Cost: regex-based highlighting is approximate; occasional mis-highlights in
  exotic languages. Accepted — every comparable tool shares this.
- Cost: no structural diff. Deliberate scope decision; a future tree-sitter
  ADR would supersede this one *only* with benchmarks and a binary-size budget.

## Alternatives considered

- **tree-sitter** — better fidelity + structural future, but binary bloat,
  per-grammar C builds, and it drags us toward a product we chose not to build.
- **No highlighting in MVP** — would torpedo the "screenshot quality" first
  impression that drives adoption of this category of tool.
- **imara-diff** for intra-line — faster on large inputs, but intra-line works
  on short strings where `similar`'s ergonomics and unicode segmentation win.
  (imara-diff remains the candidate if file-level diffing ever moves in-process.)
