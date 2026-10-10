# Deflake duplicates bounded evidence into unlimited logs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Test sweeps duplicate bounded evidence into unbounded logs.

## Details

F229 — **definite** — `deflake/bin/deflake:728` — Deflake duplicates bounded evidence into unlimited logs

The sweep's additional console capture has no cumulative size cap, and failure classification reads the entire file into
memory. The recorder's bounded artifacts and forwarding queue do not limit total bytes retained here. A noisy run can
therefore exhaust storage or fail classification. Bound duplicate capture and reads, or classify from bounded recorder
artifacts using an independently supplied run identity.

## Evidence and triage context

- deflake/bin/deflake:728-730 opens the phase log in append mode and directs subprocess stdout and stderr into it.
- deflake/bin/deflake:1179-1187 invokes the recorder through that path for each phase; :65 permits a four-hour phase.
- scripts/record-test-run.py:1386-1396 independently submits every output chunk to retained evidence and console
  forwarding.
- scripts/record-test-run.py:788-814 bounds queued chunks but continuously writes accepted chunks to stdout without a
  cumulative limit.
- deflake/bin/deflake:1200 reads the entire failed phase log; :1272 does the same for classification reruns.
- deflake/bin/deflake:551-556 locates phase logs under the retained run directory. Cleanup at :1012-1027 removes the
  workspace target directory, not these logs.
- deflake/bin/deflake:728-730 connects combined subprocess output directly to an append-only phase file.
- deflake/bin/deflake:1179-1187 sends recorded phases through that path; :65 permits four-hour phases.
- scripts/record-test-run.py:788-814 forwards offered output without a cumulative byte ceiling.
- deflake/bin/deflake:1200 and :1272 read entire phase and rerun logs into strings.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- docs/test-run-evidence.md:509-528 explicitly distinguishes capped retained output from best-effort console forwarding;
  it supplies no limit for deflake's duplicate capture.
- SPEC.md:2021-2031 accepts failures caused by unhealthy local filesystems. This finding instead concerns tooling
  consuming unbounded storage on an initially healthy filesystem.
- No matching acceptance, Planned item, BUGS.md entry, queue item, or triage decision was identified.
- SPEC.md:2021-2031 accepts failures of an already unhealthy filesystem, not tooling producing arbitrarily large logs on
  a healthy filesystem. No matching Planned, BUGS, queue or ledger disposition found.

Caveats:

- Resource exhaustion requires enough emitted output; no exhaustion was reproduced.
- Queue overflow can drop forwarded chunks, but does not impose a cumulative file-size bound.
- The confirmed consequences are storage growth and potentially excessive classification memory. Loss of unrelated user
  work is not established.
- Exhaustion depends on output volume and available resources. No exhaustion experiment performed; maintainer tooling
  only.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_sec:p1:F2`,
`test_infrastructure_04_cor:p2:F2`.

- `test_infrastructure_04_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_04_cor:p2:F2`: confidence as filed: definite; suggested bucket as filed: other.
