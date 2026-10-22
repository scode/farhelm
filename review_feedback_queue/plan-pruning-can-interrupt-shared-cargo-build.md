# Plan cleanup can interrupt another workspace's Cargo build

Reviewed commit: 343faa698aae5bf2907aa1b84ec6bc9b5a08b667

## TLDR

Closing a plan may break a build in another workspace when both use the closing checkout's build directory. Cleanup
waits for this plan's builds, but can delete compiled files another build is about to consume. The concurrent failure
schedule and its frequency have not been reproduced.

## Details

`plans/.agents/close-reset.md:79–102` directs age-based pruning of workspace artifacts after nothing the plan started is
still building or testing in this checkout. It does not require the Cargo build lock or exclude another workspace's
build. Meanwhile `.agents/machine.md:21–25` explicitly permits shared `CARGO_TARGET_DIR`, relying on Cargo's locking.
The manual deletion does not participate in that lock.

For example, another workspace can point its Cargo target directory at this checkout's `target/`. A still-current
workspace library can predate the plan-start mark. Pruning it between the other build's freshness/existence check and
compiler consumption can fail that build even though the closing plan has no active builds of its own. Cargo's ability
to rebuild a missing output on a later invocation does not establish safety during an existing invocation. No source,
retained evidence or third-party artifact loss is established by this finding.

Suggested bucket: **other**, for maintainer build disruption. Confidence: **likely**, conditional on concurrent
consumption of a pruned artifact. The documentation's guard mismatch is confirmed; no executable cleanup helper or
runtime failure was verified. A correction should account for the supported shared-directory arrangement when pruning,
rather than assuming the closing plan owns every active consumer.

Possible cover: `FILTER.md`, "Rare, self-correcting glitches and imprecise diagnostics", could cover a rare safe build
failure, but the frequency of this supported concurrent arrangement is unknown. The ledger entries
`local-desktop-build-recipe-fails-relative-target.md` and `video-recording-use-stale-builds-successful-build.md` concern
build/capture path mismatches, not concurrent pruning. Focused searches of the specifications, Planned bucket, known
bugs, queue and ledger found no exact acceptance or duplicate. The record preserves this filter ambiguity for triage.

Provenance: run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, independent cleanup auditor `g161/f2`, found during its first
discovery pass and verified by the parent against the pinned cleanup instructions. Its original exclusion of source or
evidence loss remains limited to the different released-claim, outside-descendant and evidence-retention safeguards.
