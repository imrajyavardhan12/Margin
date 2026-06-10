# ADR-0004: Four-crate workspace with one I/O seam

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

A single-crate binary is simpler today, but crate boundaries are the only
dependency boundaries the Rust compiler actually enforces. We know our growth
axes already: more diff *sources* (jj, GitHub PRs, watch mode), more *views*,
a future `--json`/library consumer of the diff model, and write operations.
Retrofitting boundaries after launch is the expensive path.

## Decision

A Cargo workspace with exactly four crates and a strict dependency direction:

```
margin (bin) ──► margin-tui ──► margin-core
        │                          ▲
        └──────► margin-vcs ───────┘
```

- **margin-core** — diff model, patch parser, intra-line diff. Pure: no I/O,
  no TUI deps, panic-free (ADR-0009).
- **margin-vcs** — the `DiffSource` trait and all implementations. The *only*
  crate that touches git, the filesystem, or stdin.
- **margin-tui** — Elm-architecture UI (ADR-0003). Depends on core only;
  never on vcs. Changesets are handed in by the binary.
- **margin** — CLI, config, wiring, the runtime shell that executes Commands.

Forbidden edges (enforced by `Cargo.toml`, checked in review):
`margin-tui ✗→ margin-vcs`, `margin-core ✗→ anything with I/O`.

## Consequences

- The compiler enforces that the UI cannot reach around the model to touch a
  repo, and that the parser cannot acquire ambient I/O.
- `margin-core` and `margin-vcs` are independently publishable; `--json`
  output and third-party tooling fall out of serializing core types.
- Tests get cheap: synthetic `DiffSource` for the TUI, no terminal for the core.
- Cost: cross-crate changes need version-lockstep (workspace versioning
  handles it) and slightly more ceremony per feature.
- Cost: four crates to publish per release; cargo-dist + CI automate it.

## Alternatives considered

- **Single crate with modules** — module privacy is advisory (any module can
  `use crate::...`); boundaries erode silently under deadline pressure.
- **More granular crates (themes, keymap, parser separately)** — premature;
  split further only when an external consumer exists (superseding ADR).
