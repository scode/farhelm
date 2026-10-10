# Replay-refusal tests do not directly observe the absence of Delete — replace with overrides

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Replace with overrides has a separate unobserved-Delete test boundary.

## Details

F285 — **possible** — `crates/farhelm-helm/src/sessions_tests.rs:3841` — Replay-refusal tests do not directly observe
the absence of Delete — replace with overrides

Its refusal and cached-source assertions do not directly observe later supervisor requests. A deletion attempt
preserving those assertions could escape, although no mutation demonstrates that sequence and current production refuses
before deletion. Extend this fixture's peer observation through an ordered boundary and assert no Delete explicitly,
separately from the plain Replace test.

## Evidence and triage context

- sessions_tests.rs:3841-3843 ends the peer after its create reply without observing subsequent DeleteSession traffic.
- sessions_tests.rs:3855-3865 asserts conflict and cached membership; :3868 joins the already-finished peer.
- rest_harness.rs:559-599 preserves the manager-side connection and answers listings separately.
- sessions.rs:1698-1700 and :3036-3045 currently enforce the shared replay veto before deletion.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"citation": "TODO.md:446-452", "reason": "Fixture migration outside Planned does not cover this negative oracle."}

Caveats:

- No current production deletion on replay refusal is established.
- A regression must attempt deletion while preserving the conflict/body/cache assertions to exploit this gap.
- That mutation was not demonstrated.
- No existing process-loss consequence is established by this test-only finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_cor_02:p1:C4`.

- `gap_helm_connections_cor_02:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
