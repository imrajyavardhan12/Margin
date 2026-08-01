# Margin documentation

Margin is a fast, keyboard-first terminal diff viewer for reviewing Git
changes, patches, and the changesets your AI agents keep producing.

```bash
brew install imrajyavardhan12/tap/margin
margin            # review the working tree
```

## Where to go

| If you want to… | Read |
|---|---|
| learn the keys | [Keybindings](keybindings.md) |
| change themes, layout, or behaviour | [Configuration](configuration.md) · [Themes](themes.md) |
| script Margin or feed an agent | [JSON output](json-output.md) |
| understand how it is built | [Architecture](architecture.md) |
| know *why* it is built that way | [Decision records](adr/README.md) |
| contribute | [Contributing](CONTRIBUTING.md) |

## The review loop

Margin exists for the case where something else wrote the code and you
have to decide whether to keep it:

1. `margin` opens the working tree — untracked files included.
2. `J` / `K` move by hunk, `]` / `[` by file, `/` searches the whole
   changeset.
3. `s` stages a hunk you accept; `x` discards one you don't (backed up
   to `.git/margin/trash/` first — `margin undo` restores it).
4. `c` leaves a note on a hunk that needs a human answer.
5. `m` checks a file off. The mark is keyed to the file's *content*, so
   it survives a rebase and clears itself if the file changes again.
6. `margin --notes` prints every note as Markdown with `path:line`
   anchors — paste it into a pull request, or hand it back to the agent.

Nothing is written to your repository unless you press a key that says
so, and the only destructive key asks you to type `yes`.
