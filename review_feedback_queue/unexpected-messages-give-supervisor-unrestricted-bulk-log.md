# Unexpected messages give a supervisor an unrestricted bulk log channel

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A supervisor can repeatedly send oversized unsolicited log messages.

## Details

F16 — **definite** — `crates/farhelm-helm/src/client.rs:1956` — Unexpected messages give a supervisor an unrestricted
bulk log channel

Unexpected supervisor messages are formatted in full, logged, and then followed by continued processing. With frames
allowed up to 8 MiB, a connected host can repeatedly cause near-frame-sized diagnostic writes, consuming shared logging
and execution resources. System-wide availability effects were not measured. Replace the full message dump with a
bounded summary and capped peer excerpt. Any additional repeated-warning budget needs a separate policy decision.

## Evidence and triage context

- crates/farhelm-proto/src/lib.rs:3373 permits unsolicited Error messages with string payloads.
- crates/farhelm-helm/src/client.rs:1751 excludes request ID zero from reply dispatch; :1956 logs the entire remaining
  message and :1960 continues.
- crates/farhelm-proto/src/lib.rs:388 permits frames up to 8 MiB.
- crates/farhelm-helm/src/manager.rs:679 already provides bounded, escaped peer diagnostics.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:1172 requires normalization wherever peer errors are logged. TRIAGE_OUTCOMES.md:28 permits repetition
  while expressly preserving bounded text. SPEC.md:2290 does not excuse an easily applied existing bound.

Caveats:

- Log throughput, contention, and system-wide availability impact were not measured.
- Debug formatting escapes controls here; this is distinct from F1.
- A per-message bound is clearly supported. Whether to introduce a repeated-warning budget needs narrower policy
  judgment.
- Debug formatting escapes controls here.
- No measured system-wide denial of service; do not claim one.
- Keep separate from the malformed-message escaping site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_01:p1:F2`.

- `gap_helm_connections_sec_01:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
