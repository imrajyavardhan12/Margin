# ADR-0012: License: MIT OR Apache-2.0

- **Status:** Accepted
- **Date:** 2026-06-10

## Context

License choice is permanent in practice (relicensing requires every
contributor's consent). Goals: maximum adoption (individuals, companies,
distro packagers), zero friction for contributors, patent safety, and
compatibility with the Rust ecosystem we depend on and hope to attract
contributors from.

## Decision

Dual-license under **MIT OR Apache-2.0** (the Rust ecosystem convention).
Both license texts ship in the repo (`LICENSE-MIT`, `LICENSE-APACHE`);
`Cargo.toml` declares `license = "MIT OR Apache-2.0"`. Contributions are
accepted under the same terms (inbound = outbound, stated in CONTRIBUTING);
no CLA.

Dependency hygiene is enforced mechanically: `cargo-deny` (deny.toml) allows
only permissive licenses (+ MPL-2.0 weak copyleft, compatible for a binary)
and fails CI otherwise.

## Consequences

- Compatible with effectively everything; packagers and corporate users need
  no review cycle.
- Apache-2.0 contributes an explicit patent grant; MIT keeps the
  maximum-simplicity option open. Users pick.
- No CLA keeps the contribution funnel frictionless; the cost is that
  relicensing later is practically impossible — we accept that permanence.
- Cost: permissive licensing allows closed-source forks. Accepted: for a
  developer tool, distribution and trust are the moat, not the license.

## Alternatives considered

- **MIT only** — fine, but drops the patent grant for no benefit.
- **GPL-3.0** — would exclude some corporate users and complicate the
  contributor funnel; the protection it buys is not where this project's risk is.
- **BSL / fair-source** — poison for a tool whose adoption depends on distro
  packaging and being recommended in dotfiles threads.
