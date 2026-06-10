<!-- PR title must be a Conventional Commit (feat:/fix:/docs:/...) — we
     squash-merge, so the title becomes the shipped commit and changelog line. -->

## What & why

<!-- One paragraph. Link the issue: Fixes #NN -->

## Checklist

- [ ] Tests: bug fixes include a test that fails before the fix; parser bugs
      add a fixture to `tests/corpus/`
- [ ] UI changes: `cargo insta review` run, snapshot diffs are intentional
- [ ] Docs updated if this adds/changes keybindings, config keys, or CLI verbs
      (`docs/keybindings.md`, `docs/configuration.md`, CHANGELOG.md)
- [ ] No accepted ADR contradicted — or a superseding ADR is included
- [ ] `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` is green locally
