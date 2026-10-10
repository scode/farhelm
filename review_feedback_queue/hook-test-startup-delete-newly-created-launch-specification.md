# Hook-test startup can delete a newly created launch specification

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Hook-test startup could delete a launch specification created during cleanup.

## Details

F263 — **possible** — `crates/farhelm/tests/e2e/hook_identity.rs:88` — Hook-test startup can delete a newly created
launch specification

The duplex test client can create a session after startup's session snapshot but before its directory sweep finishes.
The older snapshot can then classify the new launch spec as stale and delete it before the fake agent reads it. That
interleaving was not reproduced; normal socket clients wait until startup completes. Complete a handshake or request
through the actual Unix socket before returning fixture readiness.

## Evidence and triage context

- crates/farhelm/tests/e2e/hook_identity.rs:81-90 returns from ServeTask::spawn after wait_for_supervisor_ready.
  crates/farhelm/tests/e2e/harness.rs:509-516 establishes only that connect succeeds, without a protocol exchange.
- crates/farhelm/tests/e2e/harness.rs:1657-1664 constructs an already-connected client; :918-925 connects it directly to
  handle_connection through a duplex pipe, bypassing serve's accept loop.
- crates/farhelm/tests/e2e/hook_identity.rs:493-495 immediately calls hook_session after hook_harness; :160-174 creates
  through that existing client.
- crates/farhelm-supervisor/src/service/core.rs:6565 binds before :6581-6588 snapshots known sessions and sweeps launch
  artifacts. The snapshot is released before the asynchronous directory scan.
- crates/farhelm-supervisor/src/service/core.rs:13258 publishes a launch spec. service/launch_artifacts.rs:389-399 scans
  asynchronously; :443-451 removes a published JSON spec whose session is absent from the earlier snapshot.
- crates/farhelm-supervisor/src/launch.rs:874-882 returns a launch failure when the shim cannot read the deleted spec.
- harness.rs:1659–1664 creates an already-connected duplex client before ServeTask. hook_identity.rs:81–90 waits only
  for the socket-connect boundary at harness.rs:512–516. core.rs:6565 binds before taking the known-session snapshot at
  :6581–6587 and sweeping at :6588. launch_artifacts.rs:443–451 removes a JSON spec absent from that snapshot.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:565-579, sweep-deletes-live-staged-sentinel.md, protects surviving sessions' staged sentinels
  during restart. This finding concerns a newly created session's specification racing fixture startup; trigger,
  artifact and scope differ.
- SPEC_impl.md:2222-2230 and TRIAGE_OUTCOMES.md:1643-1706 permit specified obsolete-artifact cleanup and retention of
  unread current specs. They do not authorize deleting a new launch's needed specification.
- review_feedback_queue/FILTER.md:24-42 could be proposed for a timing-dependent test failure, but its application to
  this fixture race is uncertain. FILTER.md:18-20 requires retaining uncertain matches.
- TRIAGE_OUTCOMES.md:565–579 covers surviving-shim staged sentinels; :1655–1672 covers superseded generations. Neither
  covers a new post-bind session missing from the snapshot. FILTER.md:36–42 does not clearly cover deletion of a current
  artifact as stale debris; applicability is therefore unresolved, not grounds for dropping.

Caveats:

- The destructive interleaving was not reproduced.
- The demonstrated access path is test infrastructure. Shipped socket clients cannot issue requests before the accept
  loop starts.
- No loss of user-owned work or production security defect is established.
- No scheduling reproduction. This is a test-harness lifecycle race; normal socket clients are not accepted until
  startup completes. User-owned work loss outside the fixture is not established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_sec:p2:F1`,
`cli_installation_05_cor:p1:C1`.

- `cli_installation_05_sec:p2:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `cli_installation_05_cor:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
