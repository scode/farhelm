# Orphan-client test accepts failed inspection as proof of no leaked writer

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The orphan-client test accepts unknown inspection as proof of no writer leak.

## Details

F189 — **definite** — `crates/farhelm/tests/e2e/terminal_tabs.rs:2765` — Orphan-client test accepts failed inspection as
proof of no leaked writer

An unsuccessful roster or inconclusive descriptor inspection satisfies the absence-of-writers assertion. Later startup
cleanup can remove the stale client and let the remaining test pass, hiding the exact pre-cleanup leak it claims to
expose. Require a successful roster and conclusive structured inspection before asserting that no live stdin writer
remains.

## Evidence and triage context

- crates/farhelm/tests/e2e/terminal_tabs.rs:2765 parses a diagnostic roster and :2771 checks only absence of mode
  WRITE:.
- crates/farhelm/tests/e2e/terminal_tabs.rs:3037 converts query failures into text; :3059 parses that text as an empty
  PID list.
- crates/farhelm/tests/e2e/terminal_tabs.rs:3139 returns inconclusive scan text; :3267 records incomplete scans without
  failing the caller.
- crates/farhelm/tests/e2e/terminal_tabs.rs:2782 subsequently starts replacement cleanup.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- BUGS.md's accepted abrupt-death behavior does not accept a false test verdict. FILTER excludes false success.

Caveats:

- No current production descriptor leak is established.
- Inspection proves the false-pass path; no runtime failure injection was run.
- No current production fd leak is established.
- This aggregates two independently repairable observation failures: roster-query failure and inconclusive pipe-holder
  inspection. Preserve both locations or split them when filing.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_cor:p1:F2`.

- `cli_installation_09_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
