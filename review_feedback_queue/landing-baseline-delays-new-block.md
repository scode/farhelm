# Landing baseline can delay a new blocked question

Reviewed commit: d64ae3a81547729100529d210d3ea50434ff933d

## TLDR

A plan question raised during a landing round can wait another 110 minutes before automatic handling. Whether the
operational-artifact exclusion covers this scheduling defect is ambiguous.

## Details

Suggested bucket: **other**. Confidence: **confirmed prescribed sequence; actual timing unmeasured**. This concerns the
maintained monitoring procedure, not a particular report or run's compliance.

`plans/.agents/landing.md:64–66` handles blocked plans at the start of a round. At its end, `194–197` checks only for
complete plans before taking a fresh watcher baseline. A question raised during the round is already blocked at that
baseline, so `scripts/plans-queue.py:1245–1248` does not wake for it. The watcher ignores unchanged/irrelevant snapshots
(`scripts/plans-watch.sh:307–354`); the idle handler finally checks blocked plans after the configured 6,600 seconds
(`landing.md:180–205`). Add non-deferred blocked questions to the pre-sleep check. The existing idle check refutes an
indefinite stall; this record concerns the extra delay only.

Possible cover: `TRIAGE_OUTCOMES.md:20479–20495,20554–20566` excludes operational artifacts and stale report-approval
bookkeeping, but does not clearly settle this maintained scheduling consequence. The rare-glitch filter is not an exact
match: a question can arise throughout a long landing round, not merely in a narrow timing window. No matching Planned
item, unlanded plan, BUGS entry, queue item or further ledger decision was found. Introduced by this commit. Proposed
drop: none; retain coverage ambiguity. No security or user-work-loss consequence is established.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `block-answering/f3`. Source review only.
