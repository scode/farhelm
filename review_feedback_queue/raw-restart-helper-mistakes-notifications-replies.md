# Raw restart helper mistakes notifications for replies

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A restart test could mistake a valid notification for the refusal reply.

## Details

F139 — **possible** — `crates/farhelm/tests/e2e/restart_with_resume.rs:350` — Raw restart helper mistakes notifications
for replies

The raw helper returns the first control frame on a connection that can receive unsolicited session-change
notifications. If such a notification arrives before the response to its restart request, the test interprets it as the
refusal and fails on valid protocol behavior. That ordering was not reproduced. Ignore unsolicited notifications and
wait within the existing deadline for the response correlated to request 1.

## Evidence and triage context

- restart_with_resume.rs:332–351 sends request 1 and returns the first control frame without correlating it. Its caller
  at :401–409 rejects any non-Error result. The serving harness starts serve at :44; connection.rs:272–275 registers
  this full-authority connection, and hints.rs:118–124,146–158 broadcasts SessionsChanged to such connections.
- crates/farhelm/tests/e2e/restart_with_resume.rs:329-339 handshakes as helm and sends RestartSession with req_id 1;
  :350-351 returns any control message without checking its variant or request ID.
- crates/farhelm/tests/e2e/restart_with_resume.rs:368-370 starts the serving fixture and captures a conversation before
  invoking the helper; :401-409 panics unless the returned message is the expected Error.
- crates/farhelm/tests/e2e/restart_with_resume.rs:22-45 starts ServeTask.
  crates/farhelm-supervisor/src/service/core.rs:6627 starts the notification sender.
- crates/farhelm-supervisor/src/service/capture.rs:260-263 marks a change when the restart offer changes.
- crates/farhelm-supervisor/src/service/connection.rs:231 and :272-275 register an unauthenticated local full-authority
  connection for notifications.
- crates/farhelm-supervisor/src/service/hints.rs:118-124 sends SessionsChanged to every currently registered link;
  :149-158 retains pending notifications across its coalescing interval.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1288–1294 explicitly permits those notifications on every full-authority connection. Possible
  FILTER.md:24–42 coverage would require establishing that the trigger is rare and the entire consequence matches;
  notification traffic is ordinary, and rarity was not demonstrated. No exact existing item found.
- SPEC_impl.md:1288-1294 explicitly permits coalesced SessionsChanged messages on every full-authority connection. This
  validates the traffic the helper mishandles; it does not accept the test defect.
- TRIAGE_OUTCOMES.md:6379-6395, restart-with-skips-create-validation.md, covers malformed launch acceptance and durable
  session-loading failure. It does not cover a regression helper misidentifying unsolicited traffic.
- review_feedback_queue/FILTER.md:24-34 might be argued to cover a safely retriable test failure, but test-only
  applicability is uncertain; FILTER.md:18-20 prevents rejection on that uncertainty.

Caveats:

- No interleaving was reproduced or frequency measured. No shipped restart defect is established.
- The notification-before-reply interleaving and frequency remain unverified. No shipped restart defect established.
- No notification-before-response execution was reproduced.
- The defect concerns this raw test helper, not the production client's reply correlation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_06_cor:p1:F4`,
`cli_installation_06_sec:p2:F1`.

- `cli_installation_06_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_06_sec:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
