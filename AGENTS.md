# Agent briefing — Margin

Margin is a terminal diff viewer: Rust workspace, four crates, Elm-style TUI.
This file is the canonical briefing for coding agents (humans: start with
[CONTRIBUTING.md](CONTRIBUTING.md)). Dense on purpose; read top to bottom
once, then use as a reference.

Orientation order: [docs/architecture.md](docs/architecture.md) (the map) →
[docs/adr/README.md](docs/adr/README.md) (the why) → this file (the how).
**Never contradict an accepted ADR without adding a superseding one.**

## Verify exactly like CI

CI runs more than fmt/clippy/test. Run ALL of these before claiming done —
each line has failed a real push that passed the others:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps   # private doc links fail CI
cargo test --workspace
cargo bench --workspace -- --quick                            # smoke only; perf claims need real runs
# CI also runs: cargo deny check advisories licenses bans sources
# (install: brew install cargo-deny). New deps can fail license/advisory gates.
```

UI snapshots: `cargo insta review` after intentional frame changes, or
`INSTA_UPDATE=always cargo test -p margin-tui` to regenerate, then re-run
plain tests to confirm. Snapshot diffs in a PR are the UI review.

## Map

```
crates/margin-core   PURE: no I/O, no TUI deps, panic-free on untrusted input
  model.rs           Changeset > FileDiff > Hunk > Line (bytes-first: ByteStr)
  patch.rs           tolerant unified-diff parser (warnings, never errors/panics)
  intraline.rs       paired_changes + intraline_ranges (word-level emphasis)
  ansi.rs            strip_ansi (git colorizes pager output)
crates/margin-vcs    the ONLY crate doing I/O; git2 quarantined here
  git.rs             GitWorktree/GitStaged/GitShow/GitRevRange + conversion
  files.rs           TwoFiles (git2 buffer diffing, no repo)
  staging.rs         apply_patch_to_index (ADR-0013: index-only, dry-run first)
  discard.rs         worktree discard + trash + undo (ADR-0014: trash before destroy)
crates/margin-tui    Elm: AppState + Msg + update() + pure view(); NEVER imports margin-vcs
  app.rs             state, Row stream (rebuilt per layout), update()
  keymap.rs          key -> Msg, one table, no logic
  highlight.rs       HighlightCache: memoized, 400-lines/frame budget, RefCell
  theme.rs           4 built-ins + ColorMode (TrueColor/Ansi16/Monochrome)
  view/              diff.rs unified+split render, sidebar, help, style.rs compose,
                     split.rs fit_spans, mod.rs layout+status+printable()
  runtime.rs         terminal session, panic guard, poll loop, command dispatch
                     (update() returns Command; CommandExecutor impl lives in the bin)
crates/margin        bin: clap CLI (main.rs), config discovery/merge (config.rs)
```

Dependency rule (compiler-enforced, do not work around):
`margin-tui -X-> margin-vcs`; `margin-core` imports nothing with I/O.

## Hard rules

- Library crates never panic on input: no `unwrap`/`expect`/raw indexing on
  untrusted data (lints warn; CI denies warnings). Tests may panic — put
  `#![allow(clippy::unwrap_used, clippy::expect_used)]` atop test files.
- All side effects in the TUI flow through `Msg`; no I/O inside `update()`
  or `view()` (ADR-0003).
- git2 types never appear in `margin-vcs` public signatures (ADR-0005).
- Everything rendered to the terminal goes through `printable()` or
  `display_path()` — control characters are an injection vector (SECURITY.md).
  Never `String::truncate` user-derived strings (mid-char panic).
- Bug fixes land with a test that fails before the fix. Parser bugs add a
  fixture to `crates/margin-core/tests/corpus/` first (the ratchet).
- Exit codes, keybindings, config keys, and CLI verbs are stability
  surfaces: changes need docs (`docs/keybindings.md`, `docs/configuration.md`),
  a CHANGELOG entry, and for CLI verbs an ADR (ADR-0007).

## Testing playbook

| Layer | Where | How |
|---|---|---|
| Parser/model unit | `margin-core/src/*` `#[cfg(test)]` | plain asserts |
| Patch corpus | `margin-core/tests/corpus/` + `corpus.rs` | add `.patch` fixture + expectation fn; fixtures are byte-exact |
| Frame snapshots | `margin-tui/tests/ui_snapshots.rs` | render via `TestBackend`, `assert_snapshot!`; symbols only |
| Style assertions | `margin-tui/tests/theme_rendering.rs` | colors are invisible to snapshots — assert on cell styles |
| Interaction | drive `update()` with `Msg` sequences, then snapshot/assert | no TTY needed |
| VCS integration | `margin-vcs/tests/git_sources.rs` | build real temp repos with git2 (`TestRepo` helper) |
| Binary/CLI | `margin/tests/cli.rs` | `env!("CARGO_BIN_EXE_margin")`; isolate config with `.env("MARGIN_CONFIG", ...)` |
| Benchmarks | `*/benches/` (criterion) | budgets: first frame <50ms, scroll <16ms |
| Fuzz | `fuzz/fuzz_targets/` | `./fuzz/seed.sh` first, then `cargo +nightly fuzz run <parse_unified\|strip_ansi\|intraline>` (needs `cargo install cargo-fuzz`); weekly CI + smoke on parser PRs; crashes become corpus fixtures |

## Gotchas (each of these cost a debugging session)

- **Corpus fixtures are byte-exact test inputs.** `.gitattributes` pins
  `*.patch -text`. Never reformat, re-encode, or "fix" their whitespace;
  several are deliberately mail-mangled (bare empty context lines).
- **`similar`'s word tokenizer**: `1;` is one token; alignment of equal
  whitespace between runs is ambiguous. Intraline tests assert on the
  *covered text*, never on exact range indices.
- **Theme changes don't show in snapshots** (symbols identical across
  themes). Use style assertions; expect zero snapshot churn from pure
  styling work — churn means you changed content.
- **The highlight cache is stateful per hunk** (syntect parses lines in
  order) and budgeted per frame. Off-screen rows legitimately render plain
  for a frame or two; tests that need warm colors draw twice.
- **Pager passthrough is byte-identical by contract** (ADR-0007). The
  `strip_ansi` path is for *parsing only* — never let any transformation
  touch passthrough bytes. Tests compare against real `git log -p` output.
- **clap is pinned to 4.5**: 4.6.1 is broken on crates.io (missing
  clap_derive). Do not "update" it without checking the registry.
- **deny.toml ignores** (paste, bincode, yaml-rust RUSTSECs) are documented
  decisions, not oversights — read the comments before touching.
- **Wrap geometry is a two-sided contract**: `AppState::row_height` must
  predict exactly what `view/diff.rs` renders. Line text exists in one
  place (`composed_line_spans`, mirrored by `line_wrap_count` via
  `printable` + `NO_NEWLINE_SUFFIX`), the break rule in one place
  (`view::wrap::RowFill`), and split geometry in one place
  (`view::split_halves`). Change line *content* only in
  `composed_line_spans` + `line_wrap_count` together; `row_height`'s
  match is deliberately exhaustive so new `Row` variants force a height
  decision.
- **`gh run list --limit 1` races pushes**: select CI runs by
  `--workflow ci.yml --commit $(git rev-parse HEAD)` after a short sleep.
- Windows CI is real: key handling filters `KeyEventKind::Press` (Windows
  sends Release too), and autocrlf corrupts anything not guarded by
  `.gitattributes`.

## Workflow

- Issues carry acceptance criteria; comment to claim. Milestone labels
  `M1`/`M2` track v0.1/v0.2.
- Conventional Commit titles (`feat:`, `fix(scope):`, ...); squash-merge —
  the PR title becomes the shipped commit and the changelog line.
- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
- Significant decisions (reversal cost > a day, or "why is it this way?")
  get an ADR via `docs/adr/template.md`.
- If your change alters commands, architecture, conventions, or adds a
  gotcha: update this file in the same PR.
