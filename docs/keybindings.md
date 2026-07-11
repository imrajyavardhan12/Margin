# Keybindings

Margin is keyboard-first with a vim grammar. Press `?` in the app for this
list. Keybindings are a stability surface (ADR-0011): they only change with
a deprecation cycle, and custom keymaps are planned post-v0.3.

## Navigation

| Key | Action |
|---|---|
| `j` / `k`, `Down` / `Up` | move down / up one row |
| `J` / `K` | next / previous hunk |
| `]` / `[`, `Tab` / `Shift-Tab` | next / previous file |
| `gg` / `G` | jump to top / bottom |
| `Ctrl-d` / `Ctrl-u`, `PgDn` / `PgUp` | half page down / up |
| `/` | search (regex, smart-case: capitals make it case-sensitive) |
| `n` / `N` | next / previous match (wrapping) |
| `f` | fuzzy file picker (type to filter, `Up`/`Down` or `Ctrl-n`/`Ctrl-p`, `Enter` jumps, `Esc` closes) |

## View

| Key | Action |
|---|---|
| `v` | switch unified / side-by-side (pins your choice over the width-based auto layout) |
| `w` | toggle line wrap (off clips long lines; wrapped rows scroll as one unit) |
| `za` / `zA` | collapse the cursor's file / everything (vim folds). Lockfiles and generated bundles fold automatically; the `collapse` config globs extend the list. Collapsed files show a `▸` header with counts; navigation skips their bodies. |
| `b` | toggle the file sidebar |
| `?` | toggle help |
| `Esc` | close help, then clear the active search |

The layout defaults to `auto`: side-by-side when the diff pane is at least
120 columns wide, unified below that. The status bar shows `[split]` when
side-by-side is active, and `[wrap]` when line wrap is on.

## Act on the diff

| Key | Action |
|---|---|
| `s` / `u` | stage / unstage the hunk under the cursor (index-only; ADR-0013). Refusals report in the status bar. Available in git worktree and `--staged` reviews. |
| `x` | discard the hunk from the working tree (ADR-0014) — Margin's only destructive action. Opens a prompt; only typing `yes` and Enter applies it, Esc cancels. A backup patch lands in `.git/margin/trash/` first (`margin undo` restores; `discard_trash = false` opts out). Worktree reviews only. |
| `m` | mark the cursor's file viewed: sidebar checkmark + the file folds (`za` reopens it without unmarking). Marks persist per review in the data dir (`$XDG_DATA_HOME/margin`, `$MARGIN_DATA` overrides) keyed by content digest — a changed file un-views itself, so a rebase keeps only untouched files marked. Patch/pager reviews keep marks session-only. |
| `r` | reload the diff from its source (also refreshes the sidebar's staged markers) |

In worktree reviews the sidebar marks files that have staged content with a
`●` dot, so partial staging is visible at a glance.

## Session

| Key | Action |
|---|---|
| `q`, `Ctrl-c` | quit |

## Reserved (coming)

| Key | Planned action | Issue |
|---|---|---|
