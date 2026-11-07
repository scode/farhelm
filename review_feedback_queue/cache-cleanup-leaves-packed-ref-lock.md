# Interrupted cache cleanup can block later checkout launches

Reviewed commit: 7278185563e1c64d5afb2e0f5ba848f66e245b39

## TLDR

Cancelling background repository-cache cleanup can leave a Git lock that makes later fresh checkouts of that repository
fail repeatedly when an upstream branch or tag needs pruning. Manual cache repair may be needed. Existing completed
checkouts are independent of this cache and are not shown to lose work.

## Details

Suggested bucket: **high**. Confidence: confirmed conditional source route, independently corroborated; no runtime
reproduction, observed incident or frequency measurement. The assigned files remain unchanged at trailing-review tip
`49ce8cb76e9f4c841e15122a89264c7c908171dc`.

The route requires supported Git 2.43.0 with its files reference backend, automatic housekeeping actually requested,
`gc.packRefs` enabled, SIGKILL while pack-refs owns `packed-refs.lock`, and a later cache fetch with nonempty pruning.
Farhelm sets `gc.autoPackLimit=10`; more than ten unkept packs, or Git's normal loose-object threshold, can request this
housekeeping. Below-threshold cleanup, disabled housekeeping/pack-refs, and fetches with nothing to prune are outside
this finding.

`crates/farhelm-supervisor/src/spare_build.rs:99–101` selects the targetless foreground gc at `launch.rs:2211–2224`. The
helper kills its process group on owner EOF or deadline at `spare_build.rs:108–116,137–154`;
`service/spare_builder.rs:542–556` cancels running guard work. A later ordinary clone reinitializes the same cache, runs
mandatory `fetch --prune` with explicit heads/tags refspecs, and stops on failure (`launch.rs:2163–2185`). Neither that
reinitialization nor private-artifact cleanup repairs internal Git locks. Cache eviction only removes unused caches at
startup after 30 days (`repo_cache.rs:84–107`). A ready spare can temporarily bypass the affected cache.

Exact Git 2.43.0 source establishes the dependency behavior:
[gc.c:380–430,689–706](https://github.com/git/git/blob/v2.43.0/builtin/gc.c) dispatches pack-refs when housekeeping is
requested; [files-backend.c:1205–1313](https://github.com/git/git/blob/v2.43.0/refs/files-backend.c) holds its packed
lock and refuses nonempty reference deletion when acquisition fails;
[packed-backend.c:1156–1185,1215–1224](https://github.com/git/git/blob/v2.43.0/refs/packed-backend.c) acquires and
releases it. [tempfile.c:64–96,110–148](https://github.com/git/git/blob/v2.43.0/tempfile.c) relies on userspace cleanup,
which SIGKILL cannot run. [lockfile.c:75–148](https://github.com/git/git/blob/v2.43.0/lockfile.c) has no stale-owner
recovery, and [fetch.c:1364–1410,1661–1674,1787](https://github.com/git/git/blob/v2.43.0/builtin/fetch.c) propagates
pruning failure. Reinitialization through `init-db.c:237–239`, `setup.c:1888–2046,2111–2203` and
`files-backend.c:3271–3288` creates the reference directories without removing this lock. These are raw-file line
numbers; earlier browser-rendered labels in the private investigation were corrected.

Possible cover: SPEC.md's cache-error/no-fallback paragraph at `1395–1396` explains how cache failure is surfaced; it
does not explicitly accept background work leaving persistent locks. The later normal-clone availability promises at
`SPEC.md:1430–1431` and `SPEC_impl.md:4110–4111` point the other way. No exact queue, plan, ledger or filter coverage
was found. The rare self-correcting filter does not cover persistent refusal needing manual repair.

Enabled-spare builds already had the underlying hazard; the reviewed targetless mode extends it to disabled-spare and
already-ready attempts. Its empty-cache tests do not interrupt real pack-refs. A focused isolated regression would
cancel while that lock is owned, remove an upstream ref, then exercise ordinary checkout creation. Any repair must
respect Farhelm's permanent cache lock; graceful termination alone does not cover owner SIGKILL.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g016/g016-l02` and corroborating `g015/g015-l01`. These are one
root cause. Proposed drop: none. No live installation was inspected or repaired.
