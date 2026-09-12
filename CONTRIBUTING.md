# Contributing to Margin

Thanks for considering it. This document is the fast path from clone to
merged PR.

## Dev setup (3 commands)

```bash
git clone https://github.com/imrajyavardhan12/Margin && cd Margin
cargo test --workspace          # everything runs headless, no terminal needed
cargo run -p margin-review      # run the binary against this repo
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

`main` is protected: changes arrive through pull requests, required CI checks
must pass, and review conversations must be resolved. Force-pushes and branch
deletion are disabled. While Margin has one active maintainer, no external
approval is mandatory; establish one required independent approval when a
second maintainer joins. Administrative bypass is reserved for emergencies.

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
- **User-facing compatibility is protected before 1.0** (ADR-0021). CLI
  behavior, config keys, JSON schemas, default keybindings, install URLs, and
  persisted review state need compatibility tests, migration where applicable,
  documentation, and a CHANGELOG entry. Internal Rust APIs remain unstable.

## Agent-assisted contributions

Coding-agent assistance is welcome. If an agent materially helped produce a
pull request, disclose that briefly in the PR; tool names and prompt transcripts
are not required. The human contributor remains responsible for understanding,
testing, licensing, and defending every change. Agent-assisted work is held to
the same correctness, security, compatibility, and documentation standards as
any other contribution.

Unsolicited autonomous bot pull requests are not accepted unless maintainers
approved that integration in advance. A maintainer may ask the human author to
explain the design, safety invariants, or failure behavior before merging.

## Before you push

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs exactly these (plus cargo-deny and a 3-OS matrix), so green locally
means green remotely, minus OS quirks.

## Supply chain (every contributor)

Every GitHub Action in `.github/workflows/` is pinned to a full commit
SHA with a readable version comment (ADR-0024), e.g.
`actions/checkout@<40-hex> # v6`. Dependabot reads that format and
proposes SHA+comment updates like any other dependency bump — prefer
its PRs over hand-editing pins.

Rules enforced by the `action pins + lint` CI job (a required check):

- No mutable action reference (`@v6`, `@stable`, branch names) merges.
- Every pinned action keeps its version comment.
- `actionlint` passes on the five hand-written workflows.

Exceptions, by design:

- `dtolnay/rust-toolchain` has no version tags, so its `stable` /
  `master` / `nightly` installer refs are pinned to branch SHAs that
  Dependabot cannot follow — re-pin by hand when the installer needs
  it. The installed *toolchain* still floats by intent (`stable`,
  `nightly`, explicit `1.85` for MSRV).
- `release.yml` is generated: never hand-edit it. Its pins come from
  `[dist.github-action-commits]` in `dist-workspace.toml`, which
  `cargo dist generate` emits verbatim — so re-pinning it means
  updating the SHAs there and regenerating. `dist plan` runs on every
  PR and fails if the file drifts from what dist would emit, and the
  SHA check enforces pins on the generated output too. Generated
  lines carry no version comments (dist emits none); the SHAs in
  `dist-workspace.toml` are the readable record.
- `actionlint` skips `release.yml` (dist's shell style is upstream's
  to fix); the linter binary itself is a pinned download, like mdBook.

## What makes a good first PR

Issues labeled [`good first issue`](../../labels/good%20first%20issue) are
scoped to one crate and have acceptance criteria in the issue body. Comment
on the issue to claim it; ask questions there — response SLA is ~48h.

## Releasing (maintainers)

1. CI is green on `main`; the enforced perf-budget tests pass and a local
   `cargo bench` smoke shows no surprising movement against the documented
   observations (benches are informational, not gates — see the performance
   strategy in `docs/architecture.md`).
2. `git cliff --tag vX.Y.Z-rc.N` → review `CHANGELOG.md` and publish a release
   candidate through cargo-dist.
3. The Smoke workflow already proves packaged archives, installer template,
   Homebrew, completions, and man pages on all three platforms (required
   checks). Manually verify what it cannot: Windows installer execution,
   musl archive spot-checks, and installer runs against the candidate's
   own (newly published) URLs.
4. Run the release's documented beta scenario. For v0.6, record at least five
   independent testers and allow at least seven days without an unresolved
   critical or high-impact defect.
5. During the candidate period, merge only fixes and documentation. Restart the
   bake period when a high-impact fix changes runtime behavior.
6. Publish in dependency order (`margin-core`, `margin-vcs`, `margin-tui`,
   then `margin-review`) after every `cargo publish --dry-run` succeeds.
7. Tag stable `vX.Y.Z`, verify the generated Homebrew formula, and curate the
   GitHub release notes.

The executable package is `margin-review`; the installed command remains
`margin`. The crates.io package named `margin` belongs to an unrelated project
and must never appear in Margin installation instructions (ADR-0025).

## Licensing of contributions

Margin is dual-licensed MIT OR Apache-2.0 (ADR-0012). By submitting a
contribution you agree it is licensed under the same terms (inbound =
outbound). There is no CLA.

## Conduct

Be excellent to each other: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
