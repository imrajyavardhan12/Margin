# Examples

Small patches that show Margin off without needing a repository. Run
any of them from the project root (or anywhere, with full paths):

```bash
margin patch examples/rust-refactor.patch
```

| File | What to look at |
|---|---|
| `rust-refactor.patch` | Rust syntax highlighting; word-level intraline emphasis on the changed parameter name and sleep call |
| `rename-and-mode.patch` | a rename with an edit (90% similarity) plus a `chmod +x` mode-only change |
| `binary-asset.patch` | a binary file alongside a text change — no hunks to show, still listed |
| `mailbox.patch` | a `git format-patch` email: headers and diffstat are preamble, the diff opens normally |
| `unicode-paths.patch` | quoted UTF-8 paths (`docs/café menu.md`) and wide-character content |

Every example parses with zero warnings (CI asserts this). They are
also handy seeds for trying flags:

```bash
margin patch examples/rust-refactor.patch --layout split
margin patch examples/unicode-paths.patch --theme carbon
margin patch examples/mailbox.patch --json | jq .files[0].hunks
```

The `.patch` files are byte-exact (`.gitattributes` pins `-text`):
editors that "fix" trailing whitespace or line endings will corrupt
them — the blank context lines really do start with a space.
