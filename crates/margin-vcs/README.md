# margin-vcs

Version-control adapters for
[Margin](https://github.com/imrajyavardhan12/Margin): the `DiffSource`
trait plus Git (via `git2`), two-file, and patch/stdin sources, and the
index/worktree write paths (hunk staging, recoverable discard, undo).

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
