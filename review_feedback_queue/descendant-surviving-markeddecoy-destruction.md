# Descendant surviving MarkedDecoy destruction

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A decoy's sleep child could survive cleanup on non-procfs platforms.

## Details

F319 — **possible** — `crates/farhelm/tests/e2e/harness.rs:2492` — Descendant surviving MarkedDecoy destruction

The decoy kills and waits only for its direct shell. If that shell retains a separate sleep child, the outer marker
guard cannot recover it where `/proc` is unavailable, potentially leaving the test-owned sleeper for up to 120 seconds.
Shell topology was not verified; readable Linux procfs covers the ordinary marked descendant. Give the fixture portable
complete-tree ownership or force a single owned process.

## Evidence and triage context

- crates/farhelm/tests/e2e/harness.rs:2462–2472 starts sh -c 'sleep 120'; :2491–2495 kills and waits only for the owned
  direct child.
- crates/farhelm/tests/e2e/marker_model.rs:107–122 installs MarkerCleanupGuard before spawning the decoy; the test has
  no platform condition.
- crates/farhelm/tests/e2e/session_lifecycle.rs:5695 and :5737 install the same outer guard and spawn the second decoy.
- crates/farhelm/tests/e2e/harness.rs:2527–2535 delegates cleanup to marked_pids; :2544–2549 returns an empty list when
  /proc cannot be read.
- crates/farhelm/tests/e2e/main.rs:39 and :56 include both modules without platform gates.
- No exact queue or ledger coverage for this descendant-cleanup mechanism was identified. Unspecified 'previously
  reported cleanup limitations' are not a sufficient coverage basis.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The observed source establishes the missing fallback, not that the supported platform's sh necessarily leaves a
  separate child in this invocation.
- The potential survivor is a test-owned sleep bounded to 120 seconds. No security consequence or loss of user-owned
  work was established.
- On Linux with readable /proc, the outer guard addresses an already-running descendant retaining the session marker.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p2:C2`.

- `cli_installation_05_cor:p2:C2`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
