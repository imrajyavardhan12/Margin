# Margin — Project Blueprint

> A fast, keyboard-first terminal diff viewer for reviewing Git changes, patches,
> and AI-authored code. Written in Rust. Single binary. Starts instantly.

This document is the complete founding blueprint: analysis of the prior art
(Hunk), product positioning, MVP scope, architecture, repo layout, process,
and a step-by-step path from first commit to public launch.

**The actual project foundation lives in the repository root** — a compiling
Cargo workspace with governance docs, CI, and **Architecture Decision Records
in [`adr/`](adr/)** that lock in every major technical
decision below with context and alternatives considered.

**Naming note:** `margin` is available on crates.io (verified 2026-06-10).
Verify GitHub org/repo and Homebrew formula availability before committing to it.
Backup candidates also free on crates.io: `onceover`, `perusal`, `mull` (collides
with the mull mutation-testing project — avoid). The rest of this document uses
**Margin** / binary `margin`.

---

## 1. What makes Hunk useful (honest analysis)

Hunk (`modem-dev/hunk`, MIT, TypeScript/Bun on OpenTUI + Pierre diffs) earned its
audience by getting a few things genuinely right:

1. **Review-first framing.** It's not a pager or a prettifier — it treats a
   changeset as *a thing you review*, with a sidebar of files and a review
   stream. That single framing decision separates it from delta/diff-so-fancy
   (stream prettifiers) and difftastic (a better diff algorithm).
2. **Mirrors Git's mental model.** `hunk diff`, `hunk show HEAD~1`,
   `hunk patch -`, `hunk pager` — zero new concepts to learn. You can drop it
   into `core.pager` and it composes with what you already do.
3. **The AI-review wedge.** Inline agent annotations and a live agent-session
   workflow ("open Hunk, point your agent's skill at the session"). It's the
   first diff tool positioned for the *human half* of the human–agent loop,
   and that's why it took off now.
4. **Responsive layout.** Auto split/stack based on terminal width — a small
   thing that makes it feel like a designed product, not a curses demo.
5. **Meets users where they are.** Watch mode, mouse support, Jujutsu and
   Sapling auto-detection, themes via TOML, npm + brew + nix packaging.
6. **Tasteful defaults.** Syntax highlighting, line numbers, good themes out
   of the box. No configuration required to get the screenshot experience.

Its structural weaknesses (our opening):

- **Runtime weight.** Node 18+ / Bun, npm global install, a session-broker
  daemon for agent workflows. Startup and memory are JS-class, and the install
  story excludes people who don't have Node.
- **View-only.** You can look at a hunk but you cannot *act* on it — no stage,
  unstage, or discard. After reviewing, you switch tools to do anything.
- **No search, no structural diff, weaker giant-diff story.** Their own
  feature table concedes structural diffing; lockfile-heavy agent diffs are
  exactly where TUI diff viewers fall over.
- **Complexity creep.** Daemon + session broker + embeddable component API at
  v0.15 is a lot of surface area for a young project.

## 2. Features to take inspiration from (not code)

Take the *ideas*, reimplement from scratch:

| Idea | Why it matters |
|---|---|
| Git-verb CLI (`diff`, `show`, `pager`, stdin patch) | Zero learning curve; composes with git config |
| File sidebar + review stream | The core "review a changeset" interaction |
| Auto split/stack responsive layout | Feels designed; works in narrow tmux panes |
| Watch mode with auto-reload | Perfect for "agent is editing right now" |
| Pager mode with non-TTY passthrough | Safe to set as `core.pager` permanently |
| TOML config + custom themes inheriting a base | Low-effort personalization |
| Untracked files included in working-tree diff | Agents create new files constantly; `git diff` hides them |
| Honest feature-comparison table in README | Builds trust, frames the category |
| Examples directory with runnable demo diffs | Best onboarding device a diff tool can have |

Explicitly **not** copying: their branding/visual identity, OpenTUI component
API, the agent daemon/session-broker architecture (right idea, too heavy too
early), and the npm distribution model.

## 3. Where Margin improves beyond Hunk

1. **Single static Rust binary.** `brew install margin`, `cargo install margin`,
   or `curl | sh`. No runtime. Cold start target: **< 50 ms** to first paint on
   a 100-file diff. This alone is a reason to switch.
2. **Act on the diff.** Stage, unstage, and discard at hunk (later: line)
   granularity, inside the review flow. Review → decide → done, one tool.
   This is the headline feature Hunk lacks and the killer workflow for AI
   changes: *accept/reject an agent's edits hunk by hunk.*
3. **Search and jump.** `/` regex search across the changeset, `n/N` between
   matches, fuzzy file picker. Table stakes from vim/less that review TUIs
   keep forgetting.
4. **Review state.** Mark files viewed (GitHub-style), persists per
   `(repo, diff-id)` so re-running after a rebase keeps your place.
5. **Giant-diff performance as a feature.** Lazy syntax highlighting,
   virtualized rendering, auto-collapse of lockfiles/generated files
   (`linguist-generated`, configurable globs). Benchmarked in CI; the README
   publishes the numbers.
6. **Structured output for tooling.** `margin --json` emits the parsed
   changeset and (later) review decisions. Agents and scripts consume Margin
   without scraping a TTY. Our agent story is *files and exit codes*, not a
   daemon — simpler, scriptable, debuggable.
7. **Terminal citizenship.** Correct behavior in tmux/ssh/16-color terminals,
   `NO_COLOR`, light/dark detection, non-TTY passthrough, graceful wide-char
   handling. Boring, and the difference between a toy and a tool.
8. **Review notes that go somewhere.** Per-hunk notes exported as Markdown —
   paste into a PR or feed back to the agent that wrote the code.

What we deliberately skip (scope discipline): structural/AST diffing
(difftastic owns it; revisit post-1.0), commit graph/log browsing (lazygit/gitui
own it), merge-conflict resolution (separate product), embeddable UI component.

## 4. MVP feature set (v0.1.0)

Principle: **the best read-only diff review experience in a terminal, period.**
Writing operations land in v0.2 — the MVP must be flawless at viewing first,
because trust in a tool that can discard your code starts with trust in how it
displays your code.

### Inputs
- `margin` / `margin diff` — working tree vs HEAD, untracked files included
- `margin diff --staged` — index vs HEAD
- `margin show [rev]`, `margin diff A..B`, `margin diff A B -- path/`
  (revision args delegated to git semantics)
- `margin diff fileA fileB` — two arbitrary files
- `git diff | margin` / `margin patch -` — any unified diff on stdin
- `margin pager` — `core.pager` mode; **passes input through unchanged when
  stdout is not a TTY**

### UI
- File sidebar (toggle `b`): status glyph, +/- counts, viewed checkmarks
- Unified and side-by-side views; `auto` picks by width; toggle `v`
- Syntax highlighting (syntect) + intra-line word-level diff highlighting
- Line numbers, hunk headers, file headers with old/new paths and mode changes
- Collapse/expand files (`za` per file, `zA` all); auto-collapse generated files
- Renames, binary files, mode changes, submodules rendered sensibly
- Help overlay (`?`), command footer, 4 built-in themes + ANSI-16 fallback

### Navigation (keyboard-first; vim grammar)
- `j/k` line, `J/K` hunk, `]`/`[` or `Tab` file, `gg/G`, `Ctrl-d/u`
- `/` search (regex), `n/N` next/prev match
- `f` fuzzy file picker
- `m` mark file viewed (session-scoped in MVP), `r` reload, `q` quit
- `w` toggle wrap, `1..4` theme quick-switch

### Quality bars (release blockers, tested in CI)
- < 50 ms to first paint, 100-file / 10k-line diff (M-series Mac, release build)
- 250k-line diff scrolls at 60 fps; highlighting is lazy and never blocks input
- Works on macOS, Linux, Windows Terminal; tmux and ssh verified
- Zero panics on the patch-corpus fuzz set; non-UTF-8 content degrades, never crashes

### Explicitly deferred
v0.2: stage/unstage/discard hunks, watch mode, persistent viewed-state.
v0.3: review notes + Markdown export, `--json`, `gh` PR integration, jj support.
Later: mouse, line-level staging, interdiff, custom keymaps, MCP server (maybe).

## 5. Tech stack and architecture

Every row below is locked in by an ADR in `adr/` — change the ADR
to change the decision.

| Concern | Choice | ADR |
|---|---|---|
| Language | **Rust** (stable, MSRV = stable−2) | [0001](adr/0001-rust.md) |
| TUI | **ratatui + crossterm** | [0002](adr/0002-ratatui-crossterm.md) |
| App pattern | **Elm architecture**, pure core | [0003](adr/0003-elm-architecture.md) |
| Code layout | **4-crate Cargo workspace**, `DiffSource` boundary | [0004](adr/0004-workspace-crates.md) |
| Git | **git2** behind a trait | [0005](adr/0005-git2-behind-trait.md) |
| Highlight / intra-line | **syntect** + **similar** | [0006](adr/0006-syntect-similar.md) |
| CLI | git-verb subcommands, pager passthrough guarantee | [0007](adr/0007-cli-design.md) |
| Config | **TOML**, XDG + repo-local | [0008](adr/0008-config-toml.md) |
| Errors | thiserror in libs, anyhow at edge, no-panic policy | [0009](adr/0009-error-handling.md) |
| Testing | unit + insta snapshots + temp-repo integration + corpus + fuzz + bench | [0010](adr/0010-testing-strategy.md) |
| Release | cargo-dist, Conventional Commits, git-cliff | [0011](adr/0011-release-distribution.md) |
| License | **MIT OR Apache-2.0** | [0012](adr/0012-license.md) |

Why not TypeScript/OpenTUI like Hunk: we'd inherit exactly the weaknesses we're
positioning against (runtime, startup, install). Why not Go/bubbletea: viable,
but ratatui's ecosystem (syntect/similar/git2/insta) is a better fit and Rust
attracts the contributor base that builds tools like this (delta, gitui, bat).

### Architecture: Cargo workspace, Elm-shaped app

```
margin/                      # workspace root
├── crates/
│   ├── margin-core/         # PURE: no I/O, no TUI deps
│   │   ├── model.rs         # Changeset → FileDiff → Hunk → Line (+ intra-line spans)
│   │   ├── patch.rs         # unified-diff parser (fuzzed)
│   │   ├── intraline.rs     # word-level diff via `similar`
│   │   └── collapse.rs      # generated-file detection, lockfile heuristics
│   ├── margin-vcs/          # DiffSource trait + implementations
│   │   ├── source.rs        # trait DiffSource { fn load(&self) -> Changeset; fn id(&self) -> DiffId }
│   │   ├── git.rs           # git2: worktree/staged/rev-range/show (+ apply in v0.2)
│   │   ├── files.rs         # two-file comparison
│   │   └── patch_input.rs   # stdin/patch-file
│   ├── margin-tui/          # rendering + interaction, depends on core only
│   │   ├── app.rs           # Model (AppState), Msg, update() — Elm architecture
│   │   ├── view/            # sidebar, unified, side_by_side, help, picker, search
│   │   ├── highlight.rs     # lazy syntect cache (highlight on first visibility)
│   │   ├── keymap.rs        # Msg ← key event mapping (custom keymaps later = data change)
│   │   └── theme.rs         # built-ins + custom-from-TOML
│   └── margin/              # the binary
│       ├── main.rs          # clap → pick DiffSource → run TUI or passthrough
│       └── config.rs        # config discovery/merge
```

Load-bearing decisions:

1. **`margin-core` is pure and I/O-free.** The diff model, parser, and
   intra-line logic are plain functions over data — trivially unit-testable,
   fuzzable, and reusable (the future `--json` output is just serializing this
   model).
2. **`DiffSource` is the only seam to the outside world.** Git, files, and
   stdin all produce the same `Changeset`. Watch mode (v0.2) is "a source that
   can notify"; jj support (v0.3) is one new impl; tests inject synthetic
   sources.
3. **Elm architecture in the TUI**: one `AppState`, one `Msg` enum, one
   `update(state, msg)`, pure `view(state) -> Frame`. Every interaction is
   testable as `update` calls + an insta snapshot of the rendered frame. No
   scattered mutable widget state.
4. **Lazy everything for big diffs**: parse the changeset eagerly (cheap),
   highlight and intra-line-diff only what scrolls into view, cache per file.
   The UI thread never blocks; a background thread warms the cache.
5. **Write operations (v0.2) live in `margin-vcs` behind explicit,
   confirmation-gated Msgs.** Discard is destructive: it requires a typed
   confirm and (config-on by default) drops a backup patch in
   `.git/margin/trash/` for undo.

## 6. Repository structure

Implemented in the repository root:

```
margin/
├── crates/                         # (as above — compiles today)
├── tests/corpus/                   # real-world .patch fixtures: renames, binary,
│                                   #   CRLF, unicode, mode changes, 100k-line lockfile
├── benches/                        # criterion giant-diff benchmarks (issue #6)
├── fuzz/                           # cargo-fuzz parser target (issue #2)
├── examples/                       # runnable demo diffs
├── docs/
│   ├── adr/                        # Architecture Decision Records (0001–0012 + template)
│   ├── architecture.md             # the crate map, kept current
│   ├── keybindings.md  configuration.md  themes.md
├── assets/                         # demo.tape (vhs) → README GIF
├── .github/
│   ├── workflows/ci.yml            # fmt, clippy, deny, 3-OS test matrix, MSRV
│   ├── ISSUE_TEMPLATE/{bug_report.yml,feature_request.yml,config.yml}
│   ├── PULL_REQUEST_TEMPLATE.md  dependabot.yml
├── README.md  CONTRIBUTING.md  ROADMAP.md  CHANGELOG.md
├── CODE_OF_CONDUCT.md  SECURITY.md  LICENSE-MIT  LICENSE-APACHE
├── Cargo.toml                      # workspace + pinned workspace.dependencies
├── rust-toolchain.toml  rustfmt.toml  deny.toml  cliff.toml
└── AGENTS.md                       # agent briefing: build/test commands, architecture map
```

## 7. README, docs, roadmap, contributing, issue templates

All written and in place in the repository root:

- `README.md` — hero GIF slot, 30-second pitch, install, quick start,
  honest comparison table (delta, difftastic, Hunk, gitui — with credit to
  Hunk as inspiration), config, FAQ.
- `CONTRIBUTING.md` — dev setup in 3 commands, architecture + ADR
  pointers, test/snapshot workflow, commit conventions, "what makes a good PR".
- `ROADMAP.md` — v0.1 → v1.0 published in-repo (roadmap-as-promise).
- `.github/ISSUE_TEMPLATE/` — structured YAML bug/feature forms +
  config.yml routing questions to Discussions.
- `adr/` — decision log; new significant decisions require an ADR
  (process in `docs/adr/README.md`).

Docs philosophy: the README is the product page; `docs/` is the manual;
`?` in-app is the cheat sheet. Every feature PR updates all three or explains
why not (PR template enforces).

## 8. Testing, CI, release, packaging

### Testing pyramid (ADR 0010)
1. **Unit** (`margin-core`): parser, model invariants, intra-line spans,
   collapse heuristics. Target: core is ~fully covered because it's pure.
2. **Snapshot** (`margin-tui`): render known changesets at fixed sizes
   (80×24, 200×50, 40×20) via ratatui `TestBackend`, snapshot with insta.
   Keybinding flows = sequences of `update()` calls then snapshot.
3. **Integration** (`margin-vcs` + bin): create real temp git repos (staged,
   untracked, renamed, binary, submodule cases), assert the Changeset; run the
   binary with `--json`/passthrough modes and assert stdout.
4. **Corpus**: every weird real-world patch that ever breaks us gets a fixture
   and a regression test. Fuzz target runs the parser on arbitrary bytes.
5. **Benchmarks**: criterion on parse + first-frame for the giant fixture;
   CI fails on >20% regression (informational at first, blocking by v0.3).

### CI (GitHub Actions — `.github/workflows/ci.yml`)
- `check`: rustfmt, clippy `-D warnings`, cargo-deny (advisories + licenses)
- `test`: matrix {ubuntu, macos, windows} × {stable}, plus MSRV on ubuntu
- `fuzz-smoke`: 60s parser fuzz on PRs touching `margin-core` (added with issue #2)
- `bench` (main only): post numbers as commit comment
- Caching via `Swatinem/rust-cache`; total PR wall-time target < 8 min

### Release & packaging (cargo-dist, ADR 0011)
- Tag `v*` → release workflow builds: `aarch64/x86_64-apple-darwin`,
  `x86_64/aarch64-unknown-linux-musl` (static), `x86_64-pc-windows-msvc`
- Artifacts: tarballs + checksums, `curl | sh` + PowerShell installers,
  Homebrew formula pushed to `<org>/homebrew-tap`
- `cargo publish` (all crates), cargo-binstall metadata for free
- git-cliff generates CHANGELOG from Conventional Commits; release notes
  curated by hand on top
- Post-1.0 channels: AUR, nixpkgs, winget, scoop, mise — driven by community PRs

## 9. The first 10 GitHub issues

Create these on day one; they make the project look alive and give early
visitors something to grab. Each gets labels from:
`area:core` `area:tui` `area:vcs` `area:ci` `type:feat` `type:infra`
`good first issue` `help wanted` `M1` (milestone v0.1).

1. **#1 Scaffold workspace, CI, and quality gates** `type:infra` `M1`
   Cargo workspace with the four crates (stub lib.rs + doc comments stating
   each crate's contract), rustfmt/clippy/deny configs, ci.yml green, README,
   dual license, CODE_OF_CONDUCT, PR/issue templates, ADRs.
   *AC: `cargo test` green in CI on all three OSes from a fresh clone.*
   ← **largely done by this scaffold; close after first CI run.**

2. **#2 `margin-core`: changeset model + unified-diff parser** `area:core` `M1`
   Define `Changeset/FileDiff/Hunk/Line` (+ rename/binary/mode metadata).
   Parse unified diffs incl. git extensions (rename from/to, mode, binary).
   Corpus fixtures + fuzz target.
   *AC: parses every fixture byte-exactly round-trippable; fuzz 10⁶ iters clean.*

3. **#3 `margin-vcs`: `DiffSource` trait + git worktree/staged/rev sources** `area:vcs` `M1`
   git2-backed sources for worktree (untracked included), `--staged`, revision
   ranges, `show`. Integration tests on generated temp repos.
   *AC: matches `git diff` semantics on the test matrix incl. renames.*

4. **#4 Minimal TUI: file list + unified view + vim navigation** `area:tui` `M1`
   Elm skeleton (AppState/Msg/update/view), sidebar, unified diff pane,
   j/k/J/K/]/[/gg/G/q, help overlay. Insta snapshots at 3 terminal sizes.
   *AC: review a real repo's diff end-to-end with keyboard only.*

5. **#5 Side-by-side view + responsive auto layout** `area:tui` `M1`
   Split view, `v` toggle, auto mode switching on width threshold, wrap toggle.
   *AC: snapshot tests for both layouts; no panic at 40×10.*

6. **#6 Syntax highlighting + intra-line word diff, lazily** `area:tui` `area:core` `M1`
   syntect with precompiled assets, highlight-on-visibility cache,
   `similar`-based word spans. Bench: first frame of giant fixture < 50 ms.
   *AC: bench in CI; scrolling never blocks on highlighting.*

7. **#7 stdin patch mode + two-file mode + pager passthrough** `area:vcs` `M1`
   `margin patch -`, `margin diff a b`, `margin pager` with non-TTY
   passthrough. The "safe as core.pager" guarantee.
   *AC: `git -c core.pager='margin pager' log -p | cat` is byte-identical to git's output.*

8. **#8 Config file + themes (4 built-in, custom via TOML, ANSI-16 fallback)** `area:tui` `M1`
   Config discovery/merge, theme definitions, `NO_COLOR`, light/dark detection.
   *AC: docs/configuration.md + docs/themes.md written; snapshot per theme.*

9. **#9 Search (`/`, `n/N`) and fuzzy file picker (`f`)** `area:tui` `M1`
   Regex search across changeset with match highlighting; fuzzy picker over
   file paths. `good first issue` candidates inside (e.g., match-count badge).
   *AC: snapshots; search on giant fixture < 100 ms.*

10. **#10 Release pipeline + demo assets** `type:infra` `M1`
    cargo-dist config, release workflow, Homebrew tap repo, vhs tape rendering
    the README GIF, install docs.
    *AC: tagging `v0.1.0-rc1` produces installable artifacts on all 5 targets.*

(Then immediately file #11 "stage/unstage hunks" and #12 "watch mode" tagged
`M2`, so visitors see the trajectory.)

## 10. Roadmap: MVP → polished OSS launch

### Phase 0 — Foundation (week 1)
Issue #1 (this scaffold) + #2–#3 started. Private or quiet-public repo. Decide
name (crates.io ✅; check GitHub org + brew). Reserve the crate name with a
0.0.1 stub publish.

### Phase 1 — Vertical slice (weeks 2–4)
Issues #4–#7. Gate: **you stop using `git diff` yourself.** Dogfood daily;
every paper cut becomes an issue.

### Phase 2 — MVP polish (weeks 5–7)
Issues #8–#10. Windows/tmux/ssh verification pass. Fuzz + corpus hardening.
Write docs/. Render demo GIF. Tag **v0.1.0** — quietly. Post in 2–3 friendly
spots (ratatui Discord showcase, a couple of git-tooling threads) for early
feedback, not reach.

### Phase 3 — The differentiator (weeks 8–10) → v0.2.0
Stage/unstage hunk (`s`/`u`), discard with typed confirm + trash-patch undo,
watch mode, persistent viewed-state. This release is the launch payload:
"review *and act on* your agent's changes without leaving the terminal."

### Phase 4 — Launch prep (weeks 11–12)
- README final pass: GIF above the fold, comparison table, 60-second install→review
- `good first issue` × ≥6, CONTRIBUTING tested by an outsider following it cold
- Cut **v0.2.0**; verify brew/curl/cargo installs on clean machines
- Write the launch post: lead with the accept/reject-AI-changes workflow demo

### Phase 5 — Launch (week 13)
Show HN (Tue–Thu morning ET), r/rust + r/commandline + r/git (staggered,
tailored titles), lobste.rs, Terminal Trove, This Week in Rust PR.
Be present in comments all day; convert every feature request into a labeled
issue and reply with the link. Triage SLA for launch week: respond < 24 h.

### Phase 6 — Post-launch (months 4–6) → v0.3, then 1.0 trajectory
- v0.3: review notes + Markdown export, `--json`, `margin pr <n>` via `gh`, jj support
- Release cadence: minor every 4–6 weeks, patch as needed; never break keybindings without a deprecation cycle
- Community: label discipline, monthly "what's new" discussion post, recognize
  repeat contributors with triage rights
- 1.0 criteria: config/keybinding stability promise, zero known data-loss bugs
  in write paths, packaged in ≥4 ecosystems, 3 consecutive boring releases

### Success metrics (12 months)
Honest ones: you and ≥5 people you don't know use it daily (retention beats
stars); median time-to-first-response on issues < 48 h; ≥10 non-trivial outside
contributors; "margin" appears unprompted in "best terminal diff tool" threads.
