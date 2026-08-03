<!-- PR title must be a Conventional Commit (feat:/fix:/docs:/...) — we
     squash-merge, so the title becomes the shipped commit and changelog line. -->

## Summary

<!-- What changed? Keep this focused on one logical change. -->

## Why

<!-- Which user scenario, bug, issue, or documented decision does this address? -->

## Validation

<!-- List exact commands and manual scenarios. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [ ] `cargo test --workspace`
- [ ] Bug fixes include a failing-before-the-fix regression test; parser bugs add a corpus fixture
- [ ] UI changes were reviewed with `cargo insta review`; snapshot changes are intentional

## User-facing and safety impact

<!-- Note changes to CLI, config, JSON, keybindings, persistence, write paths, privacy, or docs. Write "None" when not applicable. -->

- [ ] Stable surfaces have compatibility coverage and a migration path where required (ADR-0021)
- [ ] Relevant user documentation and `CHANGELOG.md` are updated
- [ ] No accepted ADR is contradicted, or a superseding ADR is included
- [ ] `AGENTS.md` is updated if commands, architecture, conventions, or contributor gotchas changed

## Agent assistance

<!-- If a coding agent materially assisted, say so briefly. Tool names and prompt transcripts are not required. -->

- [ ] I understand this change and accept responsibility for its correctness, tests, and licensing.
