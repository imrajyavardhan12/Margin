# Contributing to Margin

Thanks for considering it. This document is the fast path from clone to
merged PR.

## Dev setup (3 commands)

```bash
git clone https://github.com/imrajyavardhan12/Margin && cd margin
cargo test --workspace          # everything runs headless, no terminal needed
cargo run -p margin             # run the binary against this repo
```

Requirements: stable Rust (rustup picks it up from `rust-toolchain.toml`).
That's it — no C toolchain gymnastics, no Node, no services.

## Orientation (10 minutes)

0. Contributing with a coding agent? Point it at [AGENTS.md](AGENTS.md) —
   the canonical agent briefing (commands, hard rules, testing playbook,
   gotchas). Claude Code picks it up automatically via CLAUDE.md.
1. [docs/architecture.md](docs/architecture.md) — the crate map and the rules
   between crates. Read this first.
2. [docs/adr/](docs/adr/) — why things are the way they are. If your change
   contradicts an accepted ADR, your PR needs a superseding ADR (see
   [docs/adr/README.md](docs/adr/README.md)). If your change *makes* a
   significant decision, add an ADR.
3. `crates/margin-core/src/lib.rs` doc comments — the data model contract.

The dependency rule, because it's the one people trip on:
**`margin-tui` never imports `margin-vcs`; `margin-core` never does I/O.**
The compiler enforces it via Cargo.toml; reviewers enforce the spirit.

## Making changes

- **Branch from `main`**, keep PRs focused — one logical change.
- **Commit/PR titles use [Conventional Commits](https://www.conventionalcommits.org)**
  (`feat:`, `fix:`, `perf:`, `docs:`, `refactor:`, `test:`, `chore:`).
  We squash-merge; your PR title becomes the commit that ships, and git-cliff
  turns it into the changelog. Write it for the changelog reader.
- **Bug fixes include a test that fails before the fix.** For parser bugs,
  add the offending patch to `tests/corpus/`. No regression test, no merge —
  this is how the corpus ratchet works.
- **UI changes**: run `cargo insta review` to update frame snapshots; the
  snapshot diff in your PR *is* the UI review. Include a before/after note.
- **New keybindings or config keys** are stability surfaces — they need a line
  in `docs/keybindings.md` / `docs/configuration.md` and a CHANGELOG entry.

## Before you push

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs exactly these (plus cargo-deny and a 3-OS matrix), so green locally
means green remotely, minus OS quirks.

## What makes a good first PR

Issues labeled [`good first issue`](../../labels/good%20first%20issue) are
scoped to one crate and have acceptance criteria in the issue body. Comment
on the issue to claim it; ask questions there — response SLA is ~48h.

## Releasing (maintainers)

1. CI green on `main`; benches within budget (ADR-0010).
2. `git cliff --tag vX.Y.Z` → review CHANGELOG.md.
3. Tag `vX.Y.Z`; cargo-dist builds artifacts, installers, and the brew formula.
4. Verify `brew install`, `curl | sh`, and the Windows zip on clean machines.
5. Curate the GitHub release notes; publish crates in dependency order
   (`core` → `vcs`/`tui` → `margin`).

## Licensing of contributions

Margin is dual-licensed MIT OR Apache-2.0 (ADR-0012). By submitting a
contribution you agree it is licensed under the same terms (inbound =
outbound). There is no CLA.

## Conduct

Be excellent to each other: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
