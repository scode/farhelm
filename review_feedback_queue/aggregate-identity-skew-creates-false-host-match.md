# Aggregate identity skew creates a false host match

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An aggregate identity mismatch remains an unresolved transition question.

## Details

F323 — **possible** — `crates/farhelm-helm/src/aggregate.rs:504` — Aggregate identity skew creates a false host match

The aggregate reads identity before snapshots and durable contents, which refutes the simple reversed-read allegation.
No concrete transition producing a false host match or wrong-target action was established, and publication coverage
remains incomplete. Identify and demonstrate a violating registry/cache transition before implementation work; only then
define the identity-consistent snapshot or validation needed. This is not a confirmed security defect.

## Evidence and triage context

- aggregate.rs:504-515 reads identities before actor snapshots, then reads durable content at 546; sessions.rs:2119-2147
  similarly separates identity, owner and display reads. These are concrete ordering defenses, not an exact
  specification acceptance of a false identity match. No matching coverage basis was established.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No violating transition or wrong-target operation confirmed.
- Do not present this as a demonstrated security defect; identify a concrete transition before implementation work.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_lifecycle:p1:C8`.

- `hc_lifecycle:p1:C8`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
