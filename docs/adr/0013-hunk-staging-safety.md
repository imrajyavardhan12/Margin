# ADR-0013: Hunk staging safety model

- **Status:** Accepted
- **Date:** 2026-07-05
- **Supersedes / Superseded by:** —

## Context

Issue #10 makes Margin act on the diff: `s` stages the hunk under the
cursor, `u` unstages it. These are Margin's first write operations, and
the review happens while agents may be editing the same files. The
constraints:

- **Zero data-loss paths.** A reviewer tool that can corrupt work is
  worse than no tool. The working tree is sacred; the index must never
  end up in a state plain git can't explain.
- The diff on screen can be stale: the file may have changed since the
  changeset was loaded (that is the normal case in agent workflows).
- ADR-0003: no side effects in `update()`; ADR-0005: git2 stays
  quarantined in margin-vcs, and margin-tui cannot depend on it.

## Decision

Staging applies **exactly the reviewed hunk bytes** to **the index
only**, via libgit2:

1. margin-core re-renders the selected hunk as a minimal single-hunk
   patch (`render_hunk_patch`) from the parsed model — the same bytes
   the reviewer saw, not a recomputed diff. Unstaging renders the
   **textual reverse** (`reversed` swaps additions/deletions and the
   before/after headers); both are pure and property-tested
   (parse∘render round-trips; reverse∘reverse is identity).
2. margin-vcs feeds those bytes to `git2::Diff::from_buffer` and
   `Repository::apply(&diff, ApplyLocation::Index)`. `Index` location
   means libgit2 structurally cannot touch the working tree. A
   `check_only` pass runs first; if the hunk no longer applies (stale
   review), the operation fails cleanly, the index is untouched, and
   the UI reports "changed since load — reload (r)".
3. The TUI requests writes as data: `update()` returns a `Command`,
   and the runtime executes it through a `CommandExecutor` trait
   implemented by the binary over margin-vcs (dependency inversion,
   same seam as `DiffSource`). Write commands are produced only by
   explicit `Msg::StageHunk`/`Msg::UnstageHunk` — never by navigation.
4. After a successful apply, the changeset reloads from the source and
   the cursor re-anchors via `locate()` (the existing layout-switch
   mechanism). v1 refuses to stage binary files and renames
   (`FileStatus::Renamed | Copied`) rather than guess header semantics.

## Consequences

- Easier: the whole write path is testable without a terminal —
  pure render/reverse in margin-core, temp-repo round-trips in
  margin-vcs asserting `git status` parity with plain git.
- Easier: "what got staged" is auditable — it is byte-for-byte the
  hunk the reviewer approved.
- Harder: hunks must apply atomically or not at all; there is no
  line-level staging yet (that is a later issue, and it composes: it
  is just a smaller rendered patch).
- Committed to: reload-on-conflict rather than merge heroics. If the
  world moved, we show the new world; we never force a stale hunk in.

## Alternatives considered

- **Hand-rolled index blob surgery** (read index blob, splice hunk,
  write blob): full control, but re-implements `git apply`'s context
  matching — exactly where index corruption bugs would live. Lost to
  libgit2's battle-tested apply.
- **`ApplyOptions::hunk_callback` filtering over a recomputed diff**:
  avoids re-rendering, but stages a *recomputed* hunk that may not be
  what the reviewer saw if the file changed. Lost on the staleness
  guarantee.
- **Shelling out to `git apply --cached`**: correct semantics, but
  breaks the vendored-libgit2/no-subprocess portability stance
  (ADR-0005) and complicates Windows.
