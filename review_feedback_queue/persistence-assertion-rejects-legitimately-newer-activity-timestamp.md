# Persistence assertion rejects a legitimately newer activity timestamp

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A newer persisted timestamp could make the persistence test time out.

## Details

F183 — **possible** — `crates/farhelm/tests/e2e/session_lifecycle.rs:322` — Persistence assertion rejects a legitimately
newer activity timestamp

The test requires exact equality with a sampled activity timestamp. Another qualifying observation can persist a
later-second value before the separate reader sees the first one, so successful persistence never satisfies that
equality and the helper times out. The interleaving was not reproduced, and filter applicability remains disputed.
Accept a stored value at least as new, or stop further activity before requiring equality.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:240 removes the activity quantum; :350 sends another nudge before
  reading the timestamp.
- crates/farhelm/tests/e2e/session_lifecycle.rs:289 opens a separate database connection; :322 accepts only equality.
- crates/farhelm-supervisor/src/service/ticker.rs:1744 updates memory before :1758 persists.
- crates/farhelm-supervisor/src/store.rs:4695 enforces monotonic persistence.
- SPEC_impl.md:1699 requires monotonic activity writes.
- crates/farhelm/tests/e2e/session_lifecycle.rs:350 sends a final nudge before sampling activity.
- crates/farhelm/tests/e2e/session_lifecycle.rs:322 requires exact equality, with a ten-second deadline at :314.
- crates/farhelm-supervisor/src/service/ticker.rs:1741 obtains a timestamp and :1758 persists it.
- review_feedback_queue/FILTER.md:24 covers narrow-timing safe failures.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified; FILTER application to test false failures is ambiguous.
- FILTER.md:24.

Caveats:

- A later-second qualifying observation must persist before the separate reader observes the earlier value.
- No timing reproduction was run.
- Requires another qualifying observation in a later timestamp second before the durable read observes the earlier
  value.
- No reproduction performed.
- Independent ui_a DROP/ui_b KEEP filter-applicability dispute remains unresolved. No split-delivery or later-timestamp
  interleaving was reproduced.
- The title overstates duration: the helper times out after ten seconds; it does not literally wait forever.
- A further activity observation must advance into a newer timestamp before the database reader observes the sampled
  value.
- No runtime reproduction.
- No reproduction; a subsequent activity observation must cross into a newer timestamp before the database read.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_cor:p1:F3`,
`cli_installation_07_sec:p1:F5`.

- `cli_installation_07_cor:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.
- `cli_installation_07_sec:p1:F5`: confidence as filed: possible; suggested bucket as filed: other.
