# ADR-0008: Configuration: TOML, XDG + repo-local

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

Users expect modern CLI tools to be configurable via a dotfile they can keep
in their dotfiles repo, with sane zero-config defaults. Teams additionally
want per-repository settings (e.g., collapse globs for generated files).

## Decision

- Format: **TOML**, parsed with serde — typo'd keys are **errors with
  suggestions**, not silent ignores.
- Discovery and precedence (later wins):
  1. built-in defaults
  2. `$XDG_CONFIG_HOME/margin/config.toml` (`~/.config/margin/` fallback;
     `%APPDATA%\margin\` on Windows)
  3. `.margin.toml` at the repository root (checked in by teams)
  4. CLI flags
- Repo-local config is **trusted for display options only** (themes, collapse
  globs, layout). Settings with side effects (anything that writes, external
  commands if ever added) are user-config-only — a checked-out repo must not
  be able to make Margin do things.
- Every option has a CLI-flag equivalent; `margin --dump-config` prints the
  merged effective config for debugging.
- Config keys are a stability surface: renames require a deprecation cycle
  (old key keeps working with a warning for ≥2 minor versions).

## Consequences

- Matches expectations set by every comparable tool (Hunk included); zero
  surprise for users.
- The display-only trust rule closes the "malicious repo configures the tool"
  hole *before* v0.2 adds write operations.
- Cost: precedence merging and deprecation tracking is real code; it lives in
  the binary crate (`config.rs`) and is unit-tested with fixture files.

## Alternatives considered

- **YAML** — whitespace-sensitive, surprising type coercions, heavier parsers.
- **Lua/embedded scripting** — powerful, but a scripting runtime in a tool
  that will hold staging powers is surface area we refuse on principle.
- **gitconfig sections** (`[margin]` in .gitconfig) — cute for git affinity,
  but stringly-typed, no nesting, and useless for the two-file/patch modes.
