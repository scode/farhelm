# A missing workspace's cleanup diagnostic is recognized despite discarded stderr

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Discarded workspace errors could prevent restoring test preferences.

## Details

F311 — **possible** — `e2e/tests/spawn.spec.ts:295` — A missing workspace's cleanup diagnostic is recognized despite
discarded stderr

Cleanup ignores the command's stderr, then tries to recognize a missing-workspace diagnosis from its error. If
recognition fails, rethrowing exits finally before removing scratch and restoring the shared command and YOLO
preferences. This is a possible cleanup-control-flow defect, separate from forgetting another registration. Retain
diagnostic output and attempt independent cleanup/restoration even when workspace forgetting fails.

## Evidence and triage context

- spawn.spec.ts:295 sets stdio to ignore; :298-302 tries to recover the missing-workspace diagnosis from the error and
  rethrows when it cannot. That throw exits finally before :307-309 removes scratch and restores commands-without-asking
  and YOLO preferences. The preferences were changed at :247-248. Retain as possible correctness; no unrelated user-file
  deletion is established by this candidate, and F1 separately covers forgetting the wrong registration.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_11_sec:p1:C6`.

- `test_infrastructure_11_sec:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
