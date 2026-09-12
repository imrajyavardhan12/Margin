# margin-tui

Elm-architecture terminal UI for
[Margin](https://github.com/imrajyavardhan12/Margin): application state,
messages, update logic, views, themes, and keymaps over
[`margin-core`](https://crates.io/crates/margin-core). Rendering is pure
and snapshot-testable; all effects return to the host as data.

This is an implementation crate for the `margin` binary (published as
[`margin-review`](https://crates.io/crates/margin-review)). Its Rust API
is **internal and unstable before 1.0** (see ADR-0021 in the repository):
depend on it at your own risk, and expect breaking changes in any minor
release.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../../LICENSE-MIT))

at your option. The license texts live in the repository root.
