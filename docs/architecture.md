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
| `margin-vcs` | `DiffSource` trait + git2/two-file/stdin sources | Yes — the only place | git2 quarantined; no git2 types in public APIs (ADR-0005) |
| `margin-tui` | Elm-architecture UI: `AppState`, `Msg`, `update`, `view`, themes, keymap | No (effects → `Command`) | Never imports `margin-vcs`; `view()` is pure → snapshot-testable (ADR-0002, 0003) |
| `margin` | CLI (clap), config merge, runtime shell executing `Command`s | Yes | Pager passthrough guarantee; exit codes are API (ADR-0007) |

## Data flow

```
CLI args + config ─► choose DiffSource ─► Changeset ─► AppState
                                              ▲            │ view()
   key events ─► keymap ─► Msg ─► update() ───┘            ▼
                            │                        terminal frame
                            └─► Commands (reload, apply, …) executed by the
                                runtime shell, results re-enter as Msgs
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

## Where things will live (as issues land)

- `margin-core`: `model.rs`, `patch.rs`, `intraline.rs`, `collapse.rs` (#2, #6)
- `margin-vcs`: `source.rs`, `git.rs`, `files.rs`, `patch_input.rs` (#3, #7)
- `margin-tui`: `app.rs`, `keymap.rs`, `theme.rs`, `highlight.rs`,
  `view/{sidebar,unified,side_by_side,help,picker,search}.rs` (#4, #5, #6, #8, #9)
- `margin`: `main.rs`, `config.rs` (#4, #7, #8)
