# ADR-0025: Publish the Margin binary as the `margin-review` package

- **Status:** Accepted
- **Date:** 2026-08-02
- **Amends:** ADR-0011 and ADR-0021

## Context

The `margin` name on crates.io was claimed by an unrelated project after
Margin's founding plan checked availability. Avoiding the registry removes a
normal installation and discovery path for Rust users, while renaming the
product and executable would discard a fitting identity before that is
necessary.

## Decision

The product and executable remain named `margin`. Its crates.io package is
named `margin-review`, so the supported registry installation command is:

```console
cargo install margin-review
```

The workspace dependency packages are published under their existing
`margin-core`, `margin-vcs`, and `margin-tui` names, in dependency order and at
the same lockstep version as `margin-review`. Installation documentation must
state plainly that `cargo install margin` installs an unrelated project.

Publishing implementation crates does not stabilize their Rust APIs before
1.0; ADR-0021's user-facing compatibility contract remains scoped to documented
Margin behavior. Normal Cargo semver rules and release notes still apply to
package consumers.

## Consequences

- Margin regains a conventional source-install and discovery channel without a
  product or command rename.
- Release candidates and stable releases must verify packaging and publish in
  dependency order after registry dry runs succeed.
- Four registry packages add release and ownership maintenance, and their names
  must be secured by a real release rather than empty placeholders.
