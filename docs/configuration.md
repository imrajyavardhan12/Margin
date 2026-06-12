# Configuration

Margin needs zero configuration; everything below is optional. Config keys
are a stability surface (ADR-0011): renames get a deprecation cycle.

## Files and precedence

Later sources win:

1. Built-in defaults
2. **User config**: `$XDG_CONFIG_HOME/margin/config.toml`
   (`~/.config/margin/config.toml` fallback; `%APPDATA%\margin\config.toml`
   on Windows). Set `$MARGIN_CONFIG` to point at an explicit file.
3. **Repo config**: `.margin.toml` at the repository root (searched upward
   from the working directory, stopping at the `.git` boundary)
4. CLI flags: `--theme`, `--layout`

Inspect the merged result with `margin --dump-config`.

Unknown or misspelled keys are **errors** with did-you-mean suggestions,
never silently ignored.

## Keys

```toml
# User config: full surface.
theme = "ledger"           # ledger | foolscap | carbon | blueprint
layout = "auto"            # auto | unified | split
include_untracked = true   # show untracked files in `margin` / `margin diff`
```

### Repo config is display-only

`.margin.toml` accepts **`theme` and `layout` only**. This is the ADR-0008
trust rule: checking out a repository must never change what Margin *does*,
only how it looks. Behavior keys in a repo config are an error.

## Environment

| Variable | Effect |
|---|---|
| `NO_COLOR` (non-empty) | Monochrome rendering: structure via bold/dim/reverse only |
| `COLORTERM=truecolor`/`24bit` | Forces truecolor themes |
| `TERM` containing `256color`, `kitty`, `ghostty`, `alacritty`, `wezterm`, `iterm` | Treated as truecolor-capable |
| anything else | 16-color-safe palette (syntax highlighting off) |
| `MARGIN_CONFIG` | Explicit user-config path (also handy in scripts/tests) |

See [themes.md](themes.md) for what each theme looks like and how the
degraded modes behave.
