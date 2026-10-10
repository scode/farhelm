# The foreign-origin test can finish observing before sending its request

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The foreign-origin test can stop watching before its Stop request occurs.

## Details

F142 — **definite** — `crates/farhelm-helm/src/sessions_tests.rs:4194` — The foreign-origin test can finish observing
before sending its request

The negative supervisor observer uses an elapsed window that can finish during delayed setup, before the foreign-origin
request is sent. Joining it afterward does not establish silence during the request. A regression that forwards Stop
while still returning 403 can escape the assertion. Keep observation active through request completion, then explicitly
finish and join it after an ordered boundary.

## Evidence and triage context

- crates/farhelm-helm/src/sessions_tests.rs:4194 starts a successful two-second silence timer before harness
  initialization at :4202 and the request at :4213.
- crates/farhelm-helm/src/rest_harness.rs:559 deliberately retains the manager-facing connection after the scripted peer
  exits.
- crates/farhelm-helm/src/middleware.rs:64 currently refuses the origin before invoking the route.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage found.

Caveats:

- The HTTP 403 assertion remains effective.
- A production origin bypass is not established.
- The false-pass window requires setup or scheduling to outlast the observation interval.
- The HTTP 403 assertion still catches an ordinary bypass that changes the response.
- A false pass needs both observer expiry and a regression that forwards work while preserving the refusal response.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_03:p1:F2`.

- `gap_helm_connections_cor_03:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
