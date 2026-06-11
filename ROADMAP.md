# Roadmap

This is a promise of direction, not of dates. Issues tagged with milestone
labels (`M1`, `M2`, …) are the source of truth for what's in flight.

## v0.1 — The best read-only diff review in a terminal (`M1`)

The MVP must be flawless at *viewing* before Margin earns the right to touch
your working tree.

- [x] Foundation: workspace, ADRs, CI, governance
- [x] Changeset model + tolerant unified-diff parser, corpus-tested
- [x] Git sources: worktree (incl. untracked), staged, revisions (#1)
- [x] TUI: sidebar + unified view + vim navigation + help (#2)
- [ ] Side-by-side view, responsive auto layout (#3)
- [ ] Syntax highlighting + word-level intra-line diff, lazy (#4)
- [ ] stdin patches, two-file mode, safe pager passthrough (#5)
- [ ] Config + 4 themes + ANSI-16/NO_COLOR fallback (#6)
- [ ] Search `/` and fuzzy file picker `f` (#7)
- [ ] Parser fuzz target + weekly fuzz CI (#8)
- [ ] Release pipeline: cargo-dist, brew tap, demo GIF (#9)

Quality bars (release blockers): < 50 ms first paint on a 100-file diff;
smooth scrolling on 250k lines; passthrough byte-identity; zero fuzz panics.

## v0.2 — Act on the diff (`M2`) — the launch release

- [ ] Stage / unstage hunks (`s` / `u`) (#10)
- [ ] Discard hunk with typed confirmation + undo patch in `.git/margin/trash/` (#11)
- [ ] Watch mode (`-w`): auto-reload on change, cursor preserved (#12)
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
