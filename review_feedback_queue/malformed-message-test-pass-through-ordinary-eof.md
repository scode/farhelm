# Malformed-message test can pass through ordinary EOF

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The malformed-message test can pass because the peer closes normally.

## Details

F145 — **definite** — `crates/farhelm-helm/src/client.rs:5177`; `crates/farhelm-helm/src/client.rs:5159` —
Malformed-message test can pass through ordinary EOF

After sending malformed input, the fixture disconnects. A client that incorrectly ignores the malformed frame still
receives EOF and fails the pending request in the accepted way. The test therefore does not prove malformed input
terminates an otherwise-live connection. Retain both peer halves until the request fails and independently observe
client-side connection termination.

## Evidence and triage context

- crates/farhelm-helm/src/client.rs:5169 writes malformed JSON, then :5177 drops both transport halves.
- crates/farhelm-helm/src/client.rs:5181 accepts any request error.
- crates/farhelm-helm/src/client.rs:1532 handles EOF and :1540 independently fails pending requests.
- client.rs:5161-5177 writes malformed control JSON and immediately drops both peer transport halves. :5181-5184 accepts
  any request error. Independently, EOF exits the reader at :1532-1540 and clears pending senders at :1585-1588. Current
  dispatch correctly propagates parse_control failure at :1739.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage found.
- No exact Planned, BUGS, queue or ledger coverage. FILTER.md:24-42 does not cover a permanently ineffective regression
  assertion.

Caveats:

- This is a test defect; current production parsing is fatal as intended.
- Dropping both halves here is distinct from fixtures retaining a generic split read half.
- Current production parsing correctly propagates the error; no mutation run performed.
- This is a regression-test defect, not an established current production failure.
- No runtime tests or mutation experiment were performed.
- TODO.md:33-43 plans moving session creation off the supervisor connection reader; its trigger, consequence and
  implementation scope do not cover this test.
- No matching acceptance or duplicate was found in SPEC.md, SPEC_impl.md, BUGS.md, the queue or TRIAGE_OUTCOMES.md.
- FILTER.md:24-42 does not cover a permanently ineffective regression assertion merely because its injected protocol
  input is unusual.
- Regression-test defect only. Production malformed-frame rejection currently works. No mutation experiment ran.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_02:p1:F1`, `hc_systems:p2:F1`.

- `gap_helm_connections_sec_02:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `hc_systems:p2:F1`: confidence as filed: Definite; confirmed by inspection; suggested bucket as filed: other.
