# Roadmap

This is a promise of direction, not of dates. Issues tagged with milestone
labels (`M1`, `M2`, …) are the source of truth for what's in flight.

## v0.1 — The best read-only diff review in a terminal (`M1`)

The MVP must be flawless at *viewing* before Margin earns the right to touch
your working tree.

- [x] Foundation: workspace, ADRs, CI, governance (issue #1)
- [ ] Changeset model + unified-diff parser, fuzzed (#2)
- [ ] Git sources: worktree (incl. untracked), staged, revisions (#3)
- [ ] TUI: sidebar + unified view + vim navigation + help (#4)
- [ ] Side-by-side view, responsive auto layout (#5)
- [ ] Syntax highlighting + word-level intra-line diff, lazy (#6)
- [ ] stdin patches, two-file mode, safe pager passthrough (#7)
- [ ] Config + 4 themes + ANSI-16/NO_COLOR fallback (#8)
- [ ] Search `/` and fuzzy file picker `f` (#9)
- [ ] Release pipeline: cargo-dist, brew tap, demo GIF (#10)

Quality bars (release blockers): < 50 ms first paint on a 100-file diff;
smooth scrolling on 250k lines; passthrough byte-identity; zero fuzz panics.

## v0.2 — Act on the diff (`M2`) — the launch release

- [ ] Stage / unstage hunks (`s` / `u`)
- [ ] Discard hunk with typed confirmation + undo patch in `.git/margin/trash/`
- [ ] Watch mode (`-w`): auto-reload on change, cursor preserved
- [ ] Persistent viewed-state per `(repo, diff-id)`

## v0.3 — Review workflows

- [ ] Per-hunk review notes, exported as Markdown (paste into a PR, or feed
      back to the agent that wrote the code)
- [ ] `--json`: structured changeset + review decisions for scripts and agents
- [ ] `margin pr <number>` via `gh`
- [ ] Jujutsu (`jj`) support as a new `DiffSource`

## Toward 1.0

Stability promise (config, keybindings, exit codes), zero known data-loss
bugs in write paths, packaged in ≥4 ecosystems (brew, AUR, nixpkgs, winget),
three consecutive boring releases.

## Explicitly out of scope

Structural/AST diffing (use [difftastic](https://github.com/Wilfred/difftastic)),
commit-graph/branch management (use [gitui](https://github.com/gitui-org/gitui)
or lazygit), merge-conflict resolution, an embeddable UI component, and any
daemon. Scope discipline is a feature.
