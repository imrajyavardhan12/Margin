# Architecture

> Decision history lives in [docs/adr/](adr/). This file is the current map.

## Crate graph

```
margin (bin) ──► margin-tui ──► margin-core
        │                          ▲
        └──────► margin-vcs ───────┘
```

| Crate | Role | May do I/O? | Key contract |
|---|---|---|---|
| `margin-core` | Diff model, unified-diff parser, intra-line diff, collapse heuristics | **No** | Pure, panic-free on untrusted input, fuzzed (ADR-0003, 0009) |
| `margin-vcs` | Read-only `DiffSource` adapters + repository write operations | Yes — VCS, files, `gh` | git2 quarantined; no git2 types in public APIs (ADR-0005, 0019) |
| `margin-tui` | Elm-architecture UI: `AppState`, `Msg`, `update`, `view`, themes, keymap | Terminal only (review effects → `Command`) | Never imports `margin-vcs`; `view()` is pure → snapshot-testable (ADR-0002, 0003) |
| `margin` | CLI, config, capability-aware Review Session, persistence, watching | Yes — runtime shell | Pager passthrough guarantee; exit codes are API (ADR-0007, 0019) |

## Data flow

```
CLI args + config ─► Review Session ─► DiffSource ─► Changeset ─► AppState
                              │                              ▲          │ view()
                              │     key events ─► Msg ─► update() ─────┘
                              │                       │                 ▼
                              └─ capabilities ◄──── Commands      terminal frame
                                                   │
                              runtime adapters execute effects; results return as Msgs
```

## The model (`margin-core`)

```
Changeset → FileDiff (paths, status, mode) → Hunk (@@ ranges) → Line
                                                                 └ intra-line spans
```

Everything downstream — rendering, search, `--json`, staging in v0.2 —
consumes this model. The parser that builds it from unified diffs is the
single most security-sensitive code in the project (stdin is untrusted) and
is correspondingly fuzzed and corpus-tested (ADR-0010).

## Performance strategy

Parse eagerly (cheap, linear). Highlight and intra-line-diff **lazily** —
only what scrolls into view, cached per file, warmed by a background thread
(ADR-0006). The input loop never blocks on rendering work. Budgets
(< 50 ms first paint on 100 files; smooth 250k-line scrolling) are encoded in
criterion benches and guarded in CI.

## Where things live

- `margin-core`: `model.rs` (bytes-first changeset model), `patch.rs`
  (tolerant unified-diff parser), `intraline.rs` (word-level emphasis),
  `ansi.rs` (escape stripping for pager input)
- `margin-vcs`: `lib.rs` (`DiffSource` trait + `SourceError`), `git.rs`
  (worktree/staged/show/range sources + git2 conversion), `files.rs`
  (two-file diffs)
- `margin-tui`: `app.rs` (`AppState`/`Msg`/`update`, per-layout `Row`
  stream), `keymap.rs`, `theme.rs` (built-ins + color modes),
  `highlight.rs` (budgeted lazy syntax/emphasis cache), `runtime.rs`
  (terminal session + panic guard),
  `view/{mod,diff,sidebar,help,split,style}.rs`
- `margin`: `main.rs` (clap dispatch, passthrough guarantee), `review.rs`
  (Review Session modes, capabilities, effect execution, watching and display),
  `config.rs` (discovery/merge, color-mode detection), `viewed.rs` and
  `notes.rs` (v0.5 persistence; consolidation tracked by #95)

For agent-oriented operational detail (commands, gotchas, testing
playbook), see [AGENTS.md](https://github.com/imrajyavardhan12/Margin/blob/main/AGENTS.md).
