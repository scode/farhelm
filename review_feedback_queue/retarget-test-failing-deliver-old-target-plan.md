# Retarget test failing to deliver an old-target plan

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The retarget test could miss a stale plan completing after invalidation.

## Details

F308 — **possible** — `e2e/tests/provisioning.spec.ts:1675` — Retarget test failing to deliver an old-target plan

Holding the request before server execution does not capture an old-target plan. The test proves immediate disappearance
of planning intent, then releases the request and rechecks already-satisfied state without witnessing completion.
Whether a broken late-response guard passes remains unverified. Capture an old-target response, retarget, deliver it
afterward, and observe completion before asserting that it stays rejected.

## Evidence and triage context

- e2e/tests/provisioning.spec.ts:1675-1679 delays route.continue before server execution, so it does not capture an
  old-target plan. At :1691-1699 it retargets and proves intent disappearance before release; :1700-1705 releases and
  checks already-satisfied UI state and request counts without an explicit completion oracle. The second leg similarly
  releases at :1742-1743 and checks counts at :1745-1746. crates/farhelm-ui/src/provisioning.rs:2057-2067 contains a
  distinct post-response epoch/binding guard. Whether a regression in that late-completion path passes the whole test
  remains unverified; uncertainty prevents endorsing the full drop.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_08_sec:p1:C5`.

- `test_infrastructure_08_sec:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
