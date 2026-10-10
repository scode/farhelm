# Replay-refusal tests do not directly observe the absence of Delete — plain replace

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Plain Replace refusal tests could miss an unintended asynchronous Delete.

## Details

F284 — **possible** — `crates/farhelm-helm/src/sessions_tests.rs:2626` — Replay-refusal tests do not directly observe
the absence of Delete — plain replace

The scripted peer stops reading before subsequent requests, while a separately supplied listing still includes the
source. An asynchronous or cancelled-after-enqueue Delete could therefore accompany the expected conflict without being
observed. Current synchronous refusal ordering is correct, and no such mutation was run. Keep the peer observing through
a refusal/drain boundary and explicitly require no Delete; this does not establish current process loss.

## Evidence and triage context

- sessions_tests.rs:2626-2630 ends the scripted peer immediately after SessionCreated without reading for DeleteSession.
- sessions_tests.rs:2639-2649 checks conflict text and membership, not subsequent outbound traffic.
- rest_harness.rs:559-574 preserves the manager-facing connection after peer completion and :582-599 supplies listings
  independently.
- sessions.rs:1698-1700 currently applies the veto before bookkeeping; :3036-3045 propagates refusal before
  finish_replacement.
- crates/farhelm-helm/src/sessions_tests.rs:2626–2630: the peer ends immediately after the replay reply without
  observing subsequent controls.
- crates/farhelm-helm/src/sessions_tests.rs:2639–2652: the test checks refusal text, a scripted listing, and the
  already-finished peer.
- crates/farhelm-helm/src/rest_harness.rs:20–35: scripted listing replies are independent of the one-shot peer and peer
  exit is not immediately propagated.
- crates/farhelm-helm/src/sessions.rs:1698–1699,3045,3144–3191: production currently vetoes before the awaited deletion
  tail.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"citation": "TODO.md:446-452", "reason": "General fixture migration outside Planned does not cover this negative
  oracle."}
- {"matched": null, "possible": null}

Caveats:

- Current production ordering refuses before deletion.
- Removing the veto outright would generally fail the status/body assertions; that is not the claimed missed regression.
- No mutation establishing a deletion attempt with preserved conflict/cache assertions was run.
- This test-only finding does not establish existing loss of user processes.
- Keep separate from the independently editable replace-with test.
- No current production delete-before-veto behavior is established.
- The open premise is an asynchronous or cancelled-after-enqueue delete attempt while the refusal path remains intact.
- Do not claim that the current synchronous finish_replacement path escapes the test.
- Independent editable location from sessions_tests.rs:2552.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_02:p1:C4`,
`gap_helm_connections_sec_05:p1:C8`.

- `gap_helm_connections_cor_02:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
- `gap_helm_connections_sec_05:p1:C8`: confidence as filed: possible; suggested bucket as filed: not separately tagged
  in candidate list.
