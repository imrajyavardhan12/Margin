# ADR-0010: Testing: snapshots, temp repos, corpus, fuzz, benches

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

TUIs are notoriously undertested because "you need a terminal to see it".
Our architecture (ADR-0003: pure `view()`, message-driven `update()`) was
chosen partly to make that excuse impossible. Separately, our two product
promises — performance and never-mangles-your-diff correctness — must be
regression-guarded mechanically, because manual QA won't scale past one
maintainer.

## Decision

Five layers, all in CI (`ci.yml`):

1. **Unit tests** in `margin-core`: parser cases, model invariants, intra-line
   spans, collapse heuristics. The crate is pure, so coverage here is cheap;
   it should stay near-total.
2. **Frame snapshots** in `margin-tui`: render fixed changesets at 80×24,
   200×50, and 40×20 via ratatui `TestBackend`; snapshot with **insta**.
   Interaction tests are `update()` message sequences followed by a snapshot.
   Snapshot review (`cargo insta review`) is the UI-change review mechanism.
3. **Integration tests** in `margin-vcs` + the binary: build real temp git
   repos (`tempfile` + git2) covering staged/unstacked/untracked/renames/
   binary/mode-change/submodule; assert produced `Changeset`s. Where cheap,
   compare against real `git diff` output as the semantic oracle (ADR-0005).
   Binary-level tests cover passthrough byte-identity (ADR-0007) and exit codes.
4. **Corpus + fuzz**: `tests/corpus/` holds real-world patch fixtures — every
   patch that ever breaks Margin gets a fixture before the fix lands (the
   regression ratchet). `cargo-fuzz` target on the parser; CI runs a 60-second
   smoke on PRs touching `margin-core`; longer runs scheduled weekly.
5. **Benchmarks**: criterion on parse-time and first-frame-time for a giant
   fixture (100-file/10k-line, and a 250k-line lockfile monster). Budgets from
   BLUEPRINT §4 are encoded in the bench harness; CI posts numbers on main,
   and regressions >20% block release tags (informational on PRs until v0.3).

Policy: a bug fix without a test that fails before the fix is an incomplete PR
(stated in CONTRIBUTING; enforced in review).

## Consequences

- UI regressions show up as readable text-frame diffs in PRs — reviewable by
  anyone, no terminal needed.
- The corpus turns every user bug report into permanent armor.
- Cost: snapshot churn when views change intentionally — `cargo insta review`
  makes this a one-command workflow; we accept the noise.
- Cost: temp-repo tests are the slowest layer (~seconds); they stay in the
  integration tier, not the unit tier, to keep `cargo test -p margin-core` instant.

## Alternatives considered

- **End-to-end PTY tests** (drive a real terminal) — flaky, slow, and TEA
  makes them mostly redundant; we keep at most one smoke test for raw-mode
  setup/teardown.
- **Coverage-percentage gates** — measures lines, not promises; the budgets
  and corpus ratchet guard what users actually feel.
