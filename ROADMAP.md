# Roadmap

Margin is becoming the best open-source review workspace for developers who
remain accountable for code produced with coding-agent assistance. Direction
comes from observed review friction and user evidence, not feature count or a
calendar commitment.

The issue tracker is the source of truth for work in progress. The
[CHANGELOG](CHANGELOG.md) records the detail of shipped releases.

## Shipped foundation

- **v0.1 — Review:** unified and split diff views, navigation, search, file
  picker, wrapping, syntax and intra-line highlighting, Git and patch inputs,
  themes, configuration, pager compatibility, fuzzing, benchmarks, and release
  distribution.
- **v0.2 — Act safely:** stage, unstage, discard with recovery, watch and
  reload, collapse generated files, persistent viewed marks, and structured
  JSON output.
- **v0.3 — Review workflows:** GitHub pull-request input through `gh`, stable
  viewed marks across changing revisions, automatic terminal theme selection,
  and hunk position feedback.
- **v0.4 — Ergonomics:** custom themes, mouse support, shell completions, a man
  page, examples, and per-invocation untracked-file control.
- **v0.5 — Feedback loop:** per-hunk review notes, Markdown export, and faster
  staged-state summaries.

## v0.6 — Trust release

v0.6 is feature-frozen. Its purpose is to make Margin safe and dependable
before broader promotion; new VCS integrations and broad UI capabilities do
not enter this release.

Priorities:

- Guarantee terminal restoration across every normal error and panic path.
- Make discard backup, apply, cleanup, and reload behavior testable as one
  transaction.
- Replace persistent unbacked discard with the explicit, invocation-only
  escape hatch defined by ADR-0017.
- Consolidate viewed marks and review notes into the versioned, atomic review
  state defined by ADR-0020, surfacing failures without interrupting review.
- Implement and test the stable `0`/`1`/`2` outcome contract from ADR-0022.
- Reconcile the README, architecture guide, crate documentation, ADR status,
  CLI help, and website with the shipped product.
- Protect the main branch and migrate every GitHub Action to the enforced,
  least-privilege SHA-pinning policy in ADR-0024.
- Publish the executable as the `margin-review` crates.io package while
  preserving the `margin` command (ADR-0025).
- Smoke-test supported installation paths and release artifacts.
- Dogfood complete agent-assisted review sessions and collect external beta
  feedback before declaring the release stable.

Architecture work belongs in v0.6 only when it directly improves a safety
invariant, testability, or the locality of correctness-critical behavior.
File-size cleanup by itself is not a release goal.

### Graduation gate

v0.6 first ships as `v0.6.0-rc.1`. Stable `v0.6.0` requires:

- clean release-artifact installation on macOS, Linux, and Windows;
- at least five independent testers completing the disposable-repository trust
  scenario (stage, unstage, discard, undo, notes, watch, terminal cleanup);
- at least seven days without an unresolved critical or high-impact defect;
- no candidate-period changes except fixes and documentation.

## After v0.6

- Jujutsu support remains the leading integration candidate, but should be
  designed with daily `jj` users rather than inferred from Git semantics.
- Additional packaging ecosystems should be added with maintainers who use
  those ecosystems, not merely to increase a badge count.
- New workflows should enter the roadmap only after a concrete review scenario
  and user need are documented.

## Product evidence

Margin has no numerical adoption targets before v0.6 establishes a meaningful
baseline. Early progress is assessed through voluntary feedback, repeated-use
stories, reported friction, trust failures, and contribution quality—not star,
download, or release-count quotas. Reconsider quantitative goals only after
v0.6 is stable and its first external feedback cycle is complete.

## Toward 1.0

- Stable configuration, keybinding, JSON-schema, and exit-code contracts.
- Zero known data-loss defects in write paths.
- Several consecutive uneventful releases used in real agent-assisted review
  workflows.
- Sustainable issue triage, contribution review, security response, and
  release ownership.
- Distribution through multiple actively maintained installation channels.

## Explicitly out of scope

Structural or AST diffing (use
[difftastic](https://github.com/Wilfred/difftastic)), commit-graph and branch
management (use [gitui](https://github.com/gitui-org/gitui) or lazygit), merge
conflict resolution, an embeddable UI component, a daemon, and built-in coding-
agent orchestration or vendor SDKs through 1.0 (ADR-0023). Scope discipline is
a feature.
