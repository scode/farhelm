# Notification stub accepts writes for the wrong session

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The notification stub accepts read marks addressed to another session.

## Details

F236 — **definite** — `e2e/tests/notifications.spec.ts:51` — Notification stub accepts writes for the wrong session

Its interceptor ignores the session identity in the request and updates the expected session's stub for any matching
route. A regression sending a mark to the other existing session can therefore satisfy recorded-write and bell
assertions. Bind stub mutation to the intended identifier, record request identities, and require that the other session
receives no mark.

## Evidence and triage context

- e2e/tests/notifications.spec.ts:40-46 binds listing data to id, but :51-57 accepts every session ID and records only
  route kind and sequence.
- e2e/tests/notifications.spec.ts:194-205 creates two real sessions and selects the other session; :214-228 confirms
  that distinction before opening the notification list.
- e2e/tests/notifications.spec.ts:243-248 and :260-274 assert the identity-free writes and the stub-derived bell state.
- crates/farhelm-helm/src/sessions.rs:2671-2701 accepts the other existing session, clamps its mark to its own newest
  notification or zero, and returns success.
- SPEC.md:1145-1154 requires closing a session's list to mark the entries it showed; SPEC_impl.md:2791-2795 makes marks
  explicitly per-session.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Concrete false-positive test oracle, not a confirmed product routing defect.
- The real route fetch does not rescue the oracle because the wrong existing session also returns success.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_08_sec:p1:F1`.

- `test_infrastructure_08_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
