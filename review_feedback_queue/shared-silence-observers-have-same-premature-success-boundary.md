# Shared silence observers have the same premature-success boundary

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Shared silence assertions can succeed without covering the operation.

## Details

F143 — **definite** — `crates/farhelm-helm/src/sessions_tests.rs:4514` — Shared silence observers have the same
premature-success boundary

The shared supervisor observer can finish its elapsed-time window before the relevant request occurs. Callers can then
report no supervisor traffic even though their refusal, consent, or wrong-host operation was never observed. Replace
timer-based success with explicit completion after the operations and an ordered drain boundary, and propagate the
observer's result into the verdict.

## Evidence and triage context

- crates/farhelm-helm/src/rest_harness.rs:1386 ends silence observation after two seconds independently of caller
  completion.
- crates/farhelm-helm/src/sessions_tests.rs:4514 starts it before setup and two refusal requests.
- crates/farhelm-helm/src/sessions_tests.rs:4557 starts it before a loop of create/replace requests.
- crates/farhelm-helm/src/rest_harness.rs:559 preserves the manager-facing connection after peer completion.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching coverage. TODO fixture-consolidation discussion does not cover this oracle contract.

Caveats:

- This is a shared helper defect, not proof that the production refusal paths currently forward forbidden requests.
- Changing the helper requires coordinating its callers so completion also accounts for queued forwarding.
- Other response assertions remain effective.
- Fix requires caller coordination and an ordered completion boundary, not simply a longer timeout.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_03:p1:F3`.

- `gap_helm_connections_cor_03:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
