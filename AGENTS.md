# Agent briefing — Margin

Margin is a terminal diff viewer in Rust (workspace of 4 crates). Read
`docs/architecture.md` for the crate map; every significant decision has an
ADR in `docs/adr/` — **do not contradict an accepted ADR without writing a
superseding one.**

## Commands

```bash
cargo test --workspace                                  # full headless test suite
cargo clippy --workspace --all-targets -- -D warnings   # lint (CI-blocking)
cargo fmt --all                                         # format
cargo insta review                                      # review UI frame snapshots
cargo run -p margin                                     # run the binary
```

## Hard rules (compiler- and review-enforced)

- `margin-core` does **no I/O** and never panics on input (no
  `unwrap`/`expect`/indexing on untrusted data — lints warn, CI denies warnings).
- `margin-tui` never depends on `margin-vcs`. Changesets are handed in.
- All side effects in the TUI flow through `Msg`/`Command` (Elm architecture,
  ADR-0003) — never do I/O inside `update()` or `view()`.
- git2 types never appear in `margin-vcs` public signatures (ADR-0005).
- Bug fixes require a failing-first test; parser bugs add a fixture to
  `tests/corpus/` (ADR-0010).
- Conventional Commit PR titles (squash-merge: title = shipped commit message).
- New keybindings/config keys/verbs are stability surfaces: update
  `docs/keybindings.md` / `docs/configuration.md`, CHANGELOG, and (for CLI
  verbs) an ADR per ADR-0007.
