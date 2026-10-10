# The browse-routing test discards its wrong-host assertion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The browse test ignores an observed wrong-host request.

## Details

F144 — **definite** — `crates/farhelm-helm/src/sessions_tests.rs:6203` — The browse-routing test discards its wrong-host
assertion

The local silence task can panic after receiving a wrongly routed browse request while the remote host still returns the
expected result. The test aborts and discards that task instead of checking its outcome, so its positive assertions can
all pass. Finish the observer after the operation and await its result, including panic, independently of fixing the
shared observation lifetime.

## Evidence and triage context

- crates/farhelm-helm/src/sessions_tests.rs:6142 spawns the local negative observer.
- crates/farhelm-helm/src/rest_harness.rs:1388 panics on an observed frame.
- crates/farhelm-helm/src/sessions_tests.rs:6203 aborts and discards the local task; :6204 joins only the remote peer.
- crates/farhelm-helm/src/sessions_tests.rs:6141–6142: the local silent supervisor is a spawned task.
- crates/farhelm-helm/src/rest_harness.rs:1386–1389: unexpected traffic panics inside that task.
- crates/farhelm-helm/src/sessions_tests.rs:6195–6204: the main test checks the remote response, aborts the local task,
  and joins only the remote task.
- crates/farhelm-testtrace/src/lib.rs:1005–1023,1127–1142: test outcome follows the body result and the runtime has no
  spawned-panic failure policy.
- crates/farhelm-helm/src/sessions.rs:433–440: production currently selects the requested host.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage found.
- {"matched": null, "possible": null}

Caveats:

- No current production misrouting is established.
- The shared observer's premature expiry is a separate defect; fixing it alone would not propagate this task's panic.
- No production misrouting demonstrated; retain separately from the shared silence-helper finding.
- Test defect, not proof of current production misrouting.
- Joining an immediately aborted checker alone would still not establish that it observed all relevant traffic.
- No current product misrouting is established.
- Joining an immediately aborted checker would still need a defined observation boundary.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_03:p1:F4`,
`gap_helm_connections_sec_06:p1:F2`.

- `gap_helm_connections_cor_03:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_helm_connections_sec_06:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
