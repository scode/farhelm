# Malformed supervisor messages inject terminal controls into logs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A malformed supervisor message can alter the helm's terminal log display.

## Details

F15 — **definite** — `crates/farhelm-helm/src/client.rs:1526` — Malformed supervisor messages inject terminal controls
into logs

An unknown message type is decoded into parser-error text and then logged without escaping its control characters or
newlines. A remote supervisor can therefore forge or alter the operator's displayed stderr output. Clipboard effects
depend on terminal policy and are not needed to establish this defect. Bound the diagnostic and escape peer-controlled
text before logging it, using the existing normalization for remote diagnostics.

## Evidence and triage context

- crates/farhelm-proto/src/lib.rs:2347 derives decoding for an internally tagged enum.
- crates/farhelm-proto/src/io.rs:242 preserves serde's error.
- [private local path] Display-formats unknown variant text.
- crates/farhelm-helm/src/client.rs:1739 propagates that error to the Display-formatted warning at :1526.
- [private local path] preserves Display through Debug.
- [private local path] formats the named error field without the message-field escape guard.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- Violates SPEC_impl.md:1172. TRIAGE_OUTCOMES.md:6940 concerns a separately fixed provisioning site, not this site.

Caveats:

- No exploit was executed.
- Clipboard effects depend on the terminal's escape-sequence policy.
- The connection closes after the malformed frame, limiting this particular site to one such warning per connection.
- No exploit executed.
- One warning per malformed-frame connection.
- Clipboard effects depend on terminal policy.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_01:p1:F1`.

- `gap_helm_connections_sec_01:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
