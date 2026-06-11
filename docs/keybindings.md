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

## View

| Key | Action |
|---|---|
| `v` | switch unified / side-by-side (pins your choice over the width-based auto layout) |
| `b` | toggle the file sidebar |
| `?` | toggle help |
| `Esc` | close help |

The layout defaults to `auto`: side-by-side when the diff pane is at least
120 columns wide, unified below that. The status bar shows `[split]` when
side-by-side is active.

## Session

| Key | Action |
|---|---|
| `q`, `Ctrl-c` | quit |

## Reserved (coming)

| Key | Planned action | Issue |
|---|---|---|
| `w` | toggle wrap | #14 |
| `/`, `n`, `N` | search | #7 |
| `f` | fuzzy file picker | #7 |
| `m` | mark file viewed | M2 |
| `s` / `u` | stage / unstage hunk | #10 |
| `x` | discard hunk (typed confirm) | #11 |
| `r` | reload | #12 |
