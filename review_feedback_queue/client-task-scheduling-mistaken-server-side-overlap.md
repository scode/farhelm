# Client task scheduling is mistaken for server-side overlap

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Client scheduling could make a serialized server pass the overlap test.

## Details

F185 — **possible** — `crates/farhelm/tests/e2e/session_lifecycle.rs:4677` — Client task scheduling is mistaken for
server-side overlap

The flag records when the Stop client task resumes, not when the server completes Stop. A serialized server can finish
Stop and deliver both replies while the cheap-request task runs before the Stop task sets its flag. The false-pass
schedule was not exercised. Observe reply order through one protocol reader or hold Stop at a controlled server-side
boundary.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:4644 stores stop_done only after the Stop caller resumes.
- crates/farhelm/tests/e2e/session_lifecycle.rs:4668 awaits the cheap request and :4677 reads that caller-owned flag.
- crates/farhelm-helm/src/client.rs:1789 delivers replies to separate oneshot consumers.
- crates/farhelm-supervisor/src/service/handlers.rs:977 sends the Stop reply after teardown.
- session_lifecycle.rs:4644-4649 sets stop_done only after the client future completes; :4660 checks the child before
  issuing the cheap request; :4668-4677 awaits the cheap response and then checks only that flag. A delayed client
  continuation could leave it false after server completion. No concrete current-scheduler false-pass execution was
  demonstrated.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TODO.md:35 plans Create/reconciliation dispatch changes, not this Stop test's ordering oracle.

Caveats:

- No serialized-handler mutant was executed.
- The adverse scheduler order remains an unverified runtime premise.
- The false-pass scheduler interleaving was not executed.
- The initial live-child check establishes that Stop was in flight earlier, not during completion of the later cheap
  request.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F5`,
`cli_installation_07_sec:p1:C3`.

- `cli_installation_07_cor:p1:F5`: confidence as filed: possible; suggested bucket as filed: other.
- `cli_installation_07_sec:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
