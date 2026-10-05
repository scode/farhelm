# Plans watcher can cache an ignore verdict against the wrong revision

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Plans watcher can cache an ignore verdict against the wrong revision.

## Details

`F53 / COR-WATCHER-REVISION-CACHE` — **definite** — `scripts/plans-watch.sh:321` — Plans watcher can cache an ignore
verdict against the wrong revision

The plans watcher uses a cheap read of the plans directory's tree hash to decide whether a more expensive queue check is
needed. After the check says there is no work worth waking for, it caches that hash as ignored. The two reads
independently resolve the moving branch: the tree read can see one revision while the queue check sees a later one.

For example, the tree read can see a new pending plan at revision A. Another executor claims it before the queue check,
so that check sees revision B and says `ignore`. The watcher stores A's tree hash with B's verdict. If the executor then
gives the plan back, the plans contents can return exactly to A. Subsequent polls see the cached hash and never recheck,
even though the plan is available again. Work can wait until the watcher's default 110-minute idle cap and restart,
contrary to the intended wake on give-back.

Resolve one immutable commit for each poll and use it for both reads. Add a controlled interleaving fixture covering
claim between reads and give-back afterward. The mismatch follows from the source, but the concurrent schedule was not
reproduced. No user-work or security loss is claimed. Proposed bucket: other. No possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `auto_general p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Concurrent schedule not reproduced, sourceestablished; no userwork/security loss.

## Filed reviewer metadata

- `auto_general p2`: confidence as filed: definite / confirmed by source; the concurrent schedule is not reproduced.
  Severity: correctness. Suggested bucket: other. Suggested bucket as filed: other.
