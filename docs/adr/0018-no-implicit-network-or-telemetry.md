# ADR-0018: No implicit network access or telemetry

- **Status:** Accepted
- **Date:** 2026-08-02
- **Complements:** ADR-0015

## Context

Margin handles private source code, repository paths, patches, and review notes.
Usage telemetry or background network behavior would create a privacy and trust
obligation disproportionate to the product evidence it provides, especially
for a tool positioned around review of agent-assisted changes.

## Decision

Margin does not collect telemetry and does not make implicit network requests.
It has no background update check, remote crash reporting, usage analytics, or
transmission of code and review data.

A remote integration may run only in response to an explicit user command and
must delegate authentication and networking to a tool the user already controls.
For example, `margin pr` invokes the user's `gh` CLI under ADR-0015; Margin does
not hold credentials or open the connection itself. Any future exception
requires a new decision and an explicit user action rather than an opt-out.

## Consequences

- Local review remains local by default, with behavior that can be understood
  from the invoked command.
- Adoption is measured through release downloads, voluntary feedback, issue
  reports, discussions, and user interviews rather than runtime instrumentation.
- Margin cannot silently check for new versions or automatically submit crash
  diagnostics.
