# Themes

Four built-ins, chosen with `theme = "<name>"` in
[config](configuration.md) or `--theme <name>`.

The default is `theme = "auto"`: Margin asks the terminal for its
background color (an OSC 11 query with a 50 ms budget) and picks
`ledger` on dark, `foolscap` on light. Terminals that don't answer —
tmux or screen without passthrough, some ssh hops, CI — fall back to
`ledger`. Setting any explicit theme, in config or with `--theme`,
skips the query entirely.

| Theme | For | Character |
|---|---|---|
| `ledger` *(auto: dark)* | dark terminals | calm green/red ink on subtle tints; base16-ocean syntax |
| `foolscap` *(auto: light)* | light terminals | dark ink on paper-colored tints; InspiredGitHub syntax |
| `carbon` | dark, high contrast | bright ink, deep tints, amber hunk headers; base16-eighties syntax |
| `blueprint` | dark, blue-tinted | the drafting-table look; Solarized syntax |

All themes share the same layout and markers — switching themes never
changes *what* you see, only the palette.

## Degraded modes

Margin degrades deliberately instead of accidentally:

- **16-color terminals** (no truecolor signal in `COLORTERM`/`TERM`): every
  theme maps to one ANSI-named-colors palette. Syntax highlighting is
  disabled — its RGB output would render as garbage — while additions,
  deletions, and intra-line emphasis (reverse video) remain.
- **`NO_COLOR`**: no color at all; structure is carried by bold (additions),
  dim (deletions), reverse (emphasis, cursor, headers), and underline
  (hunk headers).

## Custom themes

Define your own in the **user** config file (issue #15): a
`[themes.<name>]` section names a built-in `base` and overrides only the
keys you care about, then `theme = "<name>"` selects it:

```toml
theme = "mocha"

[themes.mocha]
base = "carbon"            # any built-in: ledger, foolscap, carbon, blueprint
addition = "#3ddc84"       # ink keys set the foreground
addition_tint = "#0d3318"  # tint keys set the background
syntax_theme = "base16-mocha.dark"
```

Rules:

- All colors are strict `#rrggbb`. Anything else is a config error
  naming the key, as are misspelled keys (ADR-0008).
- Unset keys inherit the base's full style — colors *and* modifiers
  (bold, italic), so overriding a color never un-bolds a header.
- Naming a custom theme after a built-in (`[themes.ledger]`) shadows
  it — including when `auto` picks it — which is how you *tweak* the
  default rather than replace it.
- Degraded modes win regardless: on a 16-color terminal or under
  `NO_COLOR` a custom theme renders as the standard degraded palette,
  never as mangled RGB.
- **User config only.** A repo-local `.margin.toml` may *select* a theme
  but never define one: a checked-out repository that could restyle
  your diff could also paint additions in the background color and hide
  injected code from review.

### Schema (a stability surface)

Ink keys (foreground): `addition`, `deletion`, `context`, `line_no`,
`hunk_header`, `meta`, `sidebar_title`, `sidebar_selected`,
`sidebar_staged`, `help_border`, `note` (review notes shown inline on a
hunk header).

Background keys: `addition_tint`, `deletion_tint`, `addition_emphasis`,
`deletion_emphasis`, `cursor_line`, `search_match`.

Two-color surfaces: `file_header_fg` / `file_header_bg`,
`status_bar_fg` / `status_bar_bg`.

Other: `base` (required), `syntax_theme` — one of the bundled syntect
themes: `InspiredGitHub`, `Solarized (dark)`, `Solarized (light)`,
`base16-eighties.dark`, `base16-mocha.dark`, `base16-ocean.dark`,
`base16-ocean.light`. An unknown name errors listing these.

These key names are stable: removing or renaming one is a breaking
change and gets a CHANGELOG entry like any CLI surface.
