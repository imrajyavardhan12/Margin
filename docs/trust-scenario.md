# v0.6 trust scenario (beta-tester guide)

~30 minutes. You need a terminal, Git, and a **disposable** practice
repository — nothing you care about. Every step below says what you
should see; anything else is a defect worth reporting.

## 0. Build the disposable repo

```bash
rm -rf /tmp/margin-beta && mkdir /tmp/margin-beta && cd /tmp/margin-beta
git init -q && git config user.email beta@example.com && git config user.name Beta
printf 'one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n' > notes.txt
git add . && git commit -qm base
printf 'one\nTWO edited\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\nELEVEN edited\ntwelve\n' > notes.txt
```

The two edits are far enough apart to form two separate hunks (steps 2–4
need both: one to act on, one left over). Confirm with
`margin diff --json | jq '.files[0].hunks | length'` → `2`.

## 1. Install (pick one)

```bash
# Homebrew (stable releases only — skips candidates)
brew install imrajyavardhan12/tap/margin

# Shell script (macOS / Linux) — replace VERSION with the candidate
curl -fsSL https://github.com/imrajyavardhan12/Margin/releases/download/VERSION/margin-review-installer.sh | sh

# Rust (any OS, including candidates)
cargo install margin-review --version VERSION
```

Confirm: `margin --version` prints the candidate version.

## 2. Stage good, leave bad (`s`)

```bash
cd /tmp/margin-beta && margin
```

Move to the first hunk (`J`/`K`), press `s`. Quit (`q`), then run
`git status`: exactly one hunk staged, the other still in the worktree.
`git diff --cached` shows only the hunk you staged.

## 3. Unstage (`u`)

```bash
margin diff --staged
```

Cursor on the staged hunk, press `u`. Quit; `git status` shows nothing
staged and both edits back in the worktree.

## 4. Discard and undo (`x`, typed `yes`, `margin undo`)

In `margin`, open the confirm prompt on a hunk with `x`. Check the
prompt **states that `margin undo` restores it**. Type `yes` + Enter.
Quit; the hunk is gone from the file. Run `margin undo` → `restored …`;
the hunk is back, byte-identical (`git diff` matches the pre-discard
state).

Then try `x` + typing `no`: nothing happens except a cancellation
message. The file is untouched.

## 5. Notes and Markdown export (`c`, `m`, `--notes`)

In `margin`: `c` on a hunk, type `needs a test`, Enter. `m` on the file
(checkmark + folds). Quit, then:

```bash
margin --notes
```

Your note prints as Markdown with a `path:line` anchor. Reopen
`margin`: the note is still there, the file still checked.

## 6. Watch mode (`-w`)

```bash
margin -w
```

From another terminal in the same repo: `printf 'seven\n' >> notes.txt`.
The review reloads within a second (status bar shows `[watch]`).
Quit with `q`.

## 7. Terminal cleanup

These must all leave a usable prompt — no garbled input, no invisible
typing, no stuck alternate screen:

1. Open `margin`, quit with `q`. Type a command: it echoes normally.
2. Open `margin`, press `Ctrl-C`. Same check.
3. `margin diff | head -c 100` exits 0 with partial output.

If the terminal ever breaks: `reset` recovers it — and that recovery
being necessary is itself the defect to report.

## 8. Report back (no private source)

Reply on the beta thread with:

- `margin --version`, OS, terminal emulator, multiplexer if any
- Which steps passed; where reality differed (copy the exact message)
- Any friction: what confused you, what you had to look up
- Trust failures (lost work, scary prompt, silent anything): report
  these immediately, separately, with full detail

Do **not** paste proprietary code, file paths from work projects, or
review notes containing either. The disposable repo above keeps
everything shareable. Margin collects no telemetry — silence from your
machine is by design; your words in the thread are the entire feedback
channel.
