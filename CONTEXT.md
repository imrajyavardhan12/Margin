# Margin Review

Margin is a review workspace where a developer evaluates a set of code changes before accepting, rejecting, or commenting on them. Its primary context is local review of changes produced with coding-agent assistance.

## Language

**Agent-assisted developer**:
A developer who remains accountable for code changes produced with help from a coding agent.
_Avoid_: Agent user, AI reviewer

**Changeset**:
The complete set of file differences presented for one review.
_Avoid_: Patch, diff

**Review**:
A developer’s evaluation of a **Changeset** before deciding which changes to keep.
_Avoid_: Diff viewing, code browsing

**Viewed mark**:
An acknowledgement that the current reviewable content of a file has been examined.
_Avoid_: Approval, completion

**Review note**:
Feedback attached to the reviewable content of one hunk.
_Avoid_: Comment, annotation

**Review state**:
The durable **Viewed marks** and **Review notes** associated with one **Changeset**.
_Avoid_: Session, cache, preferences

## Relationships

- A **Review** evaluates exactly one **Changeset** at a time.
- A **Changeset** contains zero or more files with reviewable content.
- A file can have one **Viewed mark** for its current reviewable content.
- A hunk can have zero or one **Review note** during a **Review**.
- **Review state** belongs to exactly one **Changeset** identity.
- An **Agent-assisted developer** remains responsible for every decision made during a **Review**.

## Example dialogue

> **Developer:** “Does a **Viewed mark** mean I approved the file?”
> **Domain expert:** “No. It only records that you examined the file’s current content; acceptance or rejection changes the underlying **Changeset**.”

## Flagged ambiguities

- “Diff” can mean either the complete **Changeset** or one file difference; use **Changeset** for the complete review input.
- “Viewed” does not mean approved, accepted, or unchanged; it only means examined.
