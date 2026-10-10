# A later refresh can hide the stale-refresh regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A later refresh can repair the regression before the stale-refresh test checks it.

## Details

F141 — **definite** — `crates/farhelm-helm/src/sessions_tests.rs:6931` — A later refresh can hide the stale-refresh
regression

The test can let an old reply erase the newly seeded session and then let the next fresh reply restore it before
inspecting listing and routing. Both final assertions pass even with stale-response rejection removed. Absolute request
counts also do not identify the specifically held request. Acknowledge that request and hold its successor until
stale-reply processing and both assertions finish.

## Evidence and triage context

- crates/farhelm-helm/src/sessions_tests.rs:6905 arms a hold but waits for absolute request count 2.
- crates/farhelm-helm/src/sessions_tests.rs:6919 makes subsequent replies include the new session, then releases the
  stale reply and waits for request 3 before asserting.
- crates/farhelm-helm/src/rest_harness.rs:273 accepts any count at least the requested count; :598 immediately queues
  unheld replies.
- crates/farhelm-helm/src/manager.rs:4213 prevents stale publication before :4237 replaces the cache.
- crates/farhelm-helm/src/sessions_tests.rs:6905–6906: the held reply is armed before waiting for an absolute request
  count.
- crates/farhelm-helm/src/rest_harness.rs:269–274: that observation counts cumulative fleet requests, not the newly held
  request.
- crates/farhelm-helm/src/sessions_tests.rs:6919–6933: subsequent responses are updated, the stale response is released,
  and assertions wait until the next request has started.
- crates/farhelm-helm/src/rest_harness.rs:582–599: an unheld next reply is forwarded immediately.
- crates/farhelm-helm/src/manager.rs:4213–4232: the production stale-drain guard is currently present.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2616 requires stale drains to decline publication. No matching Planned, BUGS, queue, or ledger coverage
  found.
- {"matched": null, "possible": ["SPEC_impl.md:2606–2619 requires recording mutations and rejecting drains that predate
  a seed.", "TRIAGE_OUTCOMES.md:2433–2459 disposes of production cache-race consequences, not this independently
  editable test oracle."]}

Caveats:

- Established by source inspection; no mutation test was run.
- The production epoch protection is present.
- Production protection is present; no mutation run was performed.
- No production stale-drain regression established.
- No runtime mutation experiment.
- No current production stale-drain regression is established.
- No mutation experiment was run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_03:p1:F1`,
`gap_helm_connections_sec_06:p1:F1`.

- `gap_helm_connections_cor_03:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_helm_connections_sec_06:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
