# margin

> Review changes in the margin.

**Margin is a fast, keyboard-first terminal diff viewer** for reviewing Git
changes, staged/unstaged work, patches — and the changesets your AI agents
keep producing. One static binary. No runtime. Starts before you finish
blinking.

<!-- Regenerate: cargo build --release -p margin && vhs assets/demo.tape -->
![margin demo](assets/demo.gif)

> **Status: alpha.** The reviewer works today — unified and side-by-side
> layouts, syntax highlighting with word-level emphasis, incremental regex
> search, fuzzy file jump, line wrap, four themes, and a git-verb CLI with a
> byte-identical pager mode. Staging, discard, and watch mode land in v0.2
> (see [docs/adr/](docs/adr/) for the plan). Watch the repo or grab an issue.

## Why margin

- **Instant.** Rust, single binary. First paint in ~4 ms on a 100-file/10k-line
  diff and ~15 ms on a 250k-line lockfile monster (measured, release build,
  criterion — budgets enforced in CI). Syntax highlighting is budgeted per
  frame, so no diff can ever freeze the UI.
- **Keyboard-first.** Vim-grammar navigation, `/` search across the whole
  changeset, fuzzy file jump, mark-as-viewed. Review without touching the mouse.
- **Acts on the diff** *(v0.2)*. Stage, unstage, or discard hunk-by-hunk from
  inside the review — accept/reject an agent's edits without switching tools.
- **A good terminal citizen.** Safe as `core.pager` (byte-identical
  passthrough when piped), `NO_COLOR`, 16-color fallback, tmux/ssh-clean,
  works on macOS, Linux, and Windows.

## Install

Pre-release binaries (macOS, Linux incl. static musl, Windows) ship from
[GitHub Releases](https://github.com/imrajyavardhan12/Margin/releases):

```bash
# installer script (macOS / Linux)
curl -fsSL https://github.com/imrajyavardhan12/Margin/releases/download/v0.1.0-rc.1/margin-installer.sh | sh

# or build from source
cargo install --git https://github.com/imrajyavardhan12/Margin margin
```

Windows (PowerShell):

```powershell
irm https://github.com/imrajyavardhan12/Margin/releases/download/v0.1.0-rc.1/margin-installer.ps1 | iex
```

(Pre-releases are not served by GitHub's `releases/latest` URL; these links
switch to `latest` once v0.1.0 ships.)

`brew install` and `cargo install margin` (crates.io) land with v0.1.0 — see
[ROADMAP.md](ROADMAP.md).

## Quick start

```bash
margin                        # review working-tree changes (untracked included)
margin diff --staged          # review what's staged
margin show HEAD~1            # review a commit
margin diff main..feature     # review a range
margin diff old.rs new.rs     # compare two files
git diff | margin             # review any unified diff from stdin
```

Make it your Git pager — safe even for scripted/piped git, by contract:

```bash
git config --global core.pager "margin pager"
```

## Keys

| Key | Action | Key | Action |
|---|---|---|---|
| `j` / `k` | line | `/` `n` `N` | search / next / prev |
| `J` / `K` | hunk | `f` | fuzzy file picker |
| `]` / `[` | file | `w` | wrap long lines |
| `v` | unified ⇄ side-by-side | `m`, `za` | viewed / collapse 🔜 |
| `b` | sidebar | `?` | help |

Full reference: [docs/keybindings.md](docs/keybindings.md).

## How it compares

| | margin | [Hunk](https://github.com/modem-dev/hunk) | [delta](https://github.com/dandavison/delta) | [difftastic](https://github.com/Wilfred/difftastic) | [gitui](https://github.com/gitui-org/gitui) |
|---|---|---|---|---|---|
| Interactive review UI (sidebar, viewed-state) | ✅ | ✅ | ❌ | ❌ | partial |
| Stage / unstage / discard hunks in-review | 🔜 v0.2 | ❌ | ❌ | ❌ | ✅ |
| Search across the changeset | ✅ | ❌ | via less | ❌ | ❌ |
| Single static binary, no runtime | ✅ | ❌ (Node) | ✅ | ✅ | ✅ |
| Safe `core.pager` passthrough | ✅ | ✅ | ✅ | ✅ | n/a |
| Structural (AST) diff | ❌ | ❌ | ❌ | ✅ | ❌ |
| Full repo management (log, branches, push) | ❌ | ❌ | ❌ | ❌ | ✅ |

Margin does one thing: changeset review. If you want AST diffs, use
difftastic; for whole-repo management, gitui/lazygit are excellent.

## Configuration

`~/.config/margin/config.toml` (user) and `.margin.toml` (repo, display
options only — a checked-out repo can never change Margin's behavior).
Everything has a CLI flag too; `margin --dump-config` shows the merged
result. See [docs/configuration.md](docs/configuration.md) and
[docs/themes.md](docs/themes.md).

```toml
theme = "ledger"           # ledger, foolscap, carbon, blueprint
layout = "auto"            # auto, unified, split
include_untracked = true
```

`NO_COLOR` and 16-color terminals are handled with dedicated degraded
palettes, not broken RGB.

## Contributing

The architecture is documented ([docs/architecture.md](docs/architecture.md)),
every major decision has an ADR ([docs/adr/](docs/adr/)), and the test suite
runs without a terminal. Start with
[CONTRIBUTING.md](CONTRIBUTING.md) and the
[`good first issue`](../../labels/good%20first%20issue) label.

## Acknowledgements

Margin was inspired by [Hunk](https://github.com/modem-dev/hunk)'s
review-first framing of terminal diffs, and stands on
[ratatui](https://ratatui.rs), [syntect](https://github.com/trishume/syntect),
[similar](https://github.com/mitsuhiko/similar), and
[libgit2](https://libgit2.org). No code is shared with any of the tools in the
comparison table.

## License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
