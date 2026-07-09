# ADR-0014: Discard safety model (trash before destroy)

- **Status:** Accepted
- **Date:** 2026-07-09
- **Supersedes / Superseded by:** — (complements ADR-0013)

## Context

Issue #11 adds `x`: discard the hunk under the cursor from the working
tree. This is Margin's **only destructive operation** — everything else
(staging, ADR-0013) is index-only and reversible with plain git. The
working tree may hold work that exists nowhere else, and Margin reviews
often run while agents are editing the same files. The constraints:

- **Nothing is destroyed before a copy exists.** A reviewer tool that
  loses work once is dead; "the user confirmed" is not a recovery path.
- The on-screen hunk can be stale (the file changed since load) — the
  normal case in agent workflows, per ADR-0013.
- ADR-0003/0005/0013 seams stand: no I/O in `update()`, git2 stays in
  margin-vcs, writes flow through `Command`/`CommandExecutor`, and write
  commands come only from explicit messages, never navigation.

## Decision

Discard applies the **textual reverse of exactly the reviewed hunk** to
**the working tree only**, and a forward copy of that hunk is persisted
**before** anything is applied:

1. **Trash before destroy.** The forward single-hunk patch (the same
   bytes `render_hunk_patch` produces for staging — what the reviewer
   saw) is written to `.git/margin/trash/<millis>.patch` first. If the
   trash write fails, **the discard aborts**; there is no code path
   that destroys without a backup while trash is enabled. Trash lives
   under the repository's gitdir (correct for linked worktrees), is
   plain `git apply`-able by hand, and file names are zero-padded
   epoch-millisecond timestamps (collisions bump the timestamp, never
   suffix — suffixes break lexical ordering), so "newest" is lexical
   order and undo can never restore the wrong entry.
2. **Worktree only, dry-run first.** margin-vcs applies the reversed
   patch via `Repository::apply(.., ApplyLocation::WorkDir)` after a
   `check_only` pass, mirroring ADR-0013: a stale hunk fails cleanly
   and the working tree is untouched — reload (`r`) and re-review. The
   index is never written: discarding a staged-and-modified hunk leaves
   its staged copy staged, exactly like `git restore <file>`.
3. **Typed confirmation in the TUI.** `x` opens a prompt that names the
   target; only typing `yes` and Enter issues the command (Esc or any
   other input cancels). One keystroke must never destroy.
4. **Undo is a CLI verb.** `margin undo` re-applies the newest trash
   entry to the working tree (same dry-run discipline) and deletes it
   on success; if the tree moved since, the entry is kept and the
   command fails with the patch path so the user can resolve by hand.
   The trash is config-off-able (`discard_trash = false`) for users who
   accept the risk — but only from **user** config: repo-local
   `.margin.toml` cannot touch discard settings, so a hostile repository
   can never silently disable backups (same posture as SECURITY.md's
   repo-local schema restriction).

## Consequences

- Easier: recovery is boring — a trash entry is a normal patch; even
  without Margin, `git apply .git/margin/trash/<file>.patch` restores.
- Easier: the whole path reuses ADR-0013 machinery (render + reverse,
  dry-run apply, Command/executor seam, reload + re-anchor), so the
  new surface is small and already property-tested.
- Harder: trash accumulates until undone or hand-pruned; v1 ships no
  auto-expiry (revisit if it ever bites — entries are tiny).
- Committed to: refusing rather than guessing — binary, rename, and
  unsafe-path hunks refuse exactly as staging does; stale hunks are
  never forced in.

## Alternatives considered

- **y/n confirmation**: one accidental keystroke from data loss; the
  issue demands typed confirmation and the product identity is
  safety-first. Rejected.
- **Undo inside the TUI session only** (in-memory backup): dies with
  the process — a crash after discard would lose the safety copy.
  Rejected; the trash must survive the session (and Margin itself).
- **`git stash push -p`-style backups**: stashes are visible,
  garbage-collected state with user-facing semantics we'd be
  polluting; a private trash dir under `.git/margin/` is inert and
  clearly ours.
- **Auto-undo via re-applying from the changeset model**: after a
  reload the model may no longer contain the discarded hunk; only a
  persisted copy guarantees restoration.
