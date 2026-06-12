# Themes

Four built-ins, chosen with `theme = "<name>"` in
[config](configuration.md) or `--theme <name>`.

| Theme | For | Character |
|---|---|---|
| `ledger` *(default)* | dark terminals | calm green/red ink on subtle tints; base16-ocean syntax |
| `foolscap` | light terminals | dark ink on paper-colored tints; InspiredGitHub syntax |
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

User-defined themes (TOML, inheriting a built-in base and overriding
colors) are planned — see issue #15. The theme schema is being kept small
until then so custom themes can rely on it.
