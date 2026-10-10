# Replacement tests overstate immediate-list/no-delete evidence

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Replacement tests refresh away the immediate-cache regression.

## Details

F289 — **definite** — `crates/farhelm-helm/src/sessions_tests.rs:2552` — Replacement tests overstate
immediate-list/no-delete evidence

The scripted listing is updated before mutation replies, and the test forces a refresh before asserting that no refresh
was needed. A replacement that fails to seed its result into the cache can therefore be repaired before inspection.
Assert immediate visibility before any refresh, controlling background refreshes so the mutation's own cache publication
is the only possible source.

## Evidence and triage context

- crates/farhelm-helm/src/sessions_tests.rs:2454,2475: scripted listings are changed before create/delete replies.
- crates/farhelm-helm/src/sessions_tests.rs:2552–2558: refresh_to_completion precedes the assertion claiming no refresh
  was needed.
- crates/farhelm-helm/src/rest_harness.rs:1000–1005: that helper explicitly forces refreshes.
- SPEC_impl.md:2606–2619: mutations must record their results before answering.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": null}

Caveats:

- Other replacement assertions remain useful.
- Separate assertion region from the no-delete half at sessions_tests.rs:2626.
- No current production seed failure or mutation experiment is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_05:p1:C8`.

- `gap_helm_connections_sec_05:p1:C8`: confidence as filed: definite; suggested bucket as filed: not separately tagged
  in candidate list.
