# Security Policy

## Reporting

Please report vulnerabilities privately via
**GitHub Security Advisories** ("Report a vulnerability" on the Security tab)
rather than public issues. You'll get an acknowledgement within 72 hours.

## Scope notes

Margin parses untrusted input (patches on stdin, arbitrary repository
contents) and — from v0.2 — performs write operations on the user's index and
working tree. Reports we especially care about:

- Parser crashes/hangs on crafted patches (the parser is fuzzed; new
  reproducers are gold and become corpus fixtures).
- Repo-local config (`.margin.toml`) influencing anything beyond display
  options (see ADR-0008's trust rule).
- Write operations (v0.2+) affecting paths outside the repository, or
  applying content other than what was displayed.
- Escape-sequence injection: diff content must never be able to emit raw
  control sequences through Margin's renderer.

## Supported versions

Pre-1.0: the latest minor release only.
