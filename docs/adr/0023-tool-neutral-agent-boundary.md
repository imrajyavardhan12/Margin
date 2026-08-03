# ADR-0023: Margin remains a tool-neutral human review boundary through 1.0

- **Status:** Accepted
- **Date:** 2026-08-02

## Context

Margin's primary user develops with coding-agent assistance, which creates an
obvious temptation to launch or authenticate agent products directly. Such
integration would couple Margin to vendor APIs, prompt formats, credentials,
and rapidly changing CLI behavior. It would also blur the product's role as the
independent place where a human evaluates proposed changes.

## Decision

Through 1.0, Margin does not launch, control, authenticate, or retain state for
coding agents. It consumes tool-neutral review inputs—local Git changes,
revisions, and standard patches—and emits tool-neutral feedback as Markdown or
versioned JSON.

Developers and external tools may connect those inputs and outputs to any agent
through files, pipes, or explicit orchestration outside Margin. Explicit forge
input such as `margin pr` remains permitted under ADR-0015 because it acquires a
changeset for review rather than operating a coding agent.

## Consequences

- The agent proposes changes; the developer remains accountable and uses Margin
  as the independent review boundary.
- Margin avoids vendor SDKs, prompt management, agent credentials, and an
  open-ended integration compatibility matrix.
- Agent-specific convenience must be built outside Margin through its stable
  CLI and output contracts until this decision is revisited after 1.0.
