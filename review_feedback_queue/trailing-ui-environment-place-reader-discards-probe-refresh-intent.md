# Refresh can be silently reduced to a cached read

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: clicking Refresh while a container-place read is running can leave changed
container-host readiness stale because the requested host probe is never sent. Source-confirmed through synthesis, no
execution. environment_ui.rs:693-704 captures refresh in the per-trigger closure and consumes it at :698.
reader.rs:444-456 records Explicit demand but returns false while running; :651-659 then drops the arriving closure and
reuses the old closure for subsequent reads. finish at :512-537 does honor the queued demand immediately, so this is not
a lost demand flag or missing retry: the wrong operation (cached GET) answers it. ContainersSettings :52-63 leaves
Refresh enabled during reads (only deletion busy disables it), making the overlap a shipped control sequence.
api/environments.rs:6-14 is cached GET; :103-112 alone sends the explicit probe POSTs. hosts.rs:4294-4339 auto-probes
only never-checked hosts, not checked hosts whose cached capability reason has become stale. A concrete source route
needs no failed request, recipe job or missing event: after opening probes settle, a fleet notice starts a cached GET
for an already checked host; a Refresh click during that GET queues only another cached GET. Subsequent feed/fallback
reads also carry false. GET success hides any preceding request error but proves no fresh probe. The related failure
variant remains: true is consumed before awaits, so a failed explicit refresh can retry as GET without completing the
probe POSTs. Recovery and limits: Refresh during a reader backoff claims a new reader and keeps true; Refresh when idle
also work

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f1`.

### g077/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: clicking Refresh while a container-place read is
running can leave changed container-host readiness stale because the requested host probe is never sent.
Source-confirmed through synthesis, no execution. environment_ui.rs:693-704 captures refresh in the per-trigger closure
and consumes it at :698. reader.rs:444-456 records Explicit demand but returns false while running; :651-659 then drops
the arriving closure and reuses the old closure for subsequent reads. finish at :512-537 does honor the queued demand
immediately, so this is not a lost demand flag or missing retry: the wrong operation (cached GET) answers it.
ContainersSettings :52-63 leaves Refresh enabled during reads (only deletion busy disables it), making the overlap a
shipped control sequence. api/environments.rs:6-14 is cached GET; :103-112 alone sends the explicit probe POSTs.
hosts.rs:4294-4339 auto-probes only never-checked hosts, not checked hosts whose cached capability reason has become
stale. A concrete source route needs no failed request, recipe job or missing event: after opening probes settle, a
fleet notice starts a cached GET for an already checked host; a Refresh click during that GET queues only another cached
GET. Subsequent feed/fallback reads also carry false. GET success hides any preceding request error but proves no fresh
probe. The related failure variant remains: true is consumed before awaits, so a failed explicit refresh can retry as
GET without completing the probe POSTs. Recovery and limits: Refresh during a reader backoff claims a new reader and
keeps true; Refresh when idle also works. Closing/reopening Settings or opening a new launcher mounts a fresh hook and
requests true. Settings section switching only hides its mounted components, so it does not rerun the mount refresh.
New/unprobed hosts may still auto-probe on GET. An already admitted probe may independently complete and mask the issue;
the established route uses a previously settled cache. The 300-second probe freshness bound is checked by backend reuse,
not by the cached display GET, so it is not a timed UI repair. Contract: SPEC_impl.md, Podman places, creation and
images, and hook docs :684-688 distinguish cached reads from opening/explicit probe refresh. Suggested bucket: other;
intermittent ineffective refresh and stale readiness, with no established security or work-loss route.
Could-be-highest:no. proposed_drop:false. possible_cover: parent-provided g060/l1 and g061/l5 are related but distinct:
RecipeList's place_status is a separate Resource for /api/recipe-places (tensorlake/recipes.rs:375-385); job completion
invalidation concerns registry removal (tensorlake/build.rs:446-449). Neither is required for this
/api/environment-places plus probe-POST failure. Fix direction: retain probe-refresh demand in component-owned state
sampled by each dispatch, and clear it only at the intended completion boundary, rather than keeping its only bit in a
discarded/spent trigger closure. This preserves the original stale-readiness consequence without claiming permanent
unavailability.

Returned proposal (claims-050, event 10): **retain-verified**, suggested bucket **other**.

Source establishes that an explicit probe refresh overlapping a cached read is answered by another cached read. This is
stale readiness and ineffective Refresh, with no established security or work-loss route. No execution was performed.

Original stronger consequence: Original and latest records concern stale readiness and an ineffective explicit refresh;
neither establishes security harm or loss of user-owned work.

Unresolved premise or limitation: No runtime ordering or frequency measurement. Those are not needed for the conditional
closure-loss conclusion; no stronger impact route is established.

Proposed basis: Retain the source-confirmed refresh-intent loss, including the failed-refresh retry variant and ordinary
recovery limits.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from
claims-050 after inspecting it. Source establishes that an explicit probe refresh overlapping a cached read is answered
by another cached read. This is stale readiness and ineffective Refresh, with no established security or work-loss
route. No execution was performed. Retain the source-confirmed refresh-intent loss, including the failed-refresh retry
variant and ordinary recovery limits. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/environment_ui.rs:693-711 captures refresh in each
arriving closure and consumes it before awaiting; :713-728 sends true on opening/Refresh and false on feed/fallback
reads.","Same revision:crates/farhelm-ui/src/reader.rs:444-456 records Explicit demand while running but declines the
new caller; :651-665 discards that caller's closure and reuses the original closure. :517-535 dispatches the queued
demand, so the defect is operation choice rather than missing demand.","Same
revision:crates/farhelm-ui/src/containers_settings.rs:53-62 disables Refresh only for deletion busy, allowing overlap.
api/environments.rs:6-14 is GET-only; :103-112 sends the probe POSTs. crates/farhelm-helm/src/hosts.rs:4294-4306
auto-probes only hosts without checked_at, so an already-checked changed host is affected.","The spent-true closure also
makes an automatic retry after refresh failure perform GET only. An idle explicit click or remount requests true again;
an existing independent probe can mask the issue. No permanent inability to probe is established. The range's
environment_ui.rs diff leaves this hook unchanged."] Current coverage/results: ["Recording main
24bc058066fd57848e788b465719b0e6da2f98f7: SPEC.md Container sessions and SPEC_impl.md Podman places, creation and images
distinguish background probes and cached availability; neither accepts discarding explicit refresh intent.","TODO.md
Planned is empty. Searched plans/queue for probe/refresh/readiness routes; relevant podman-environments work is already
landed (plans/REPORTS.md:21), and no unlanded exact fix was found.","Searched BUGS.md, the complete INDEX.md and active
TRIAGE_OUTCOMES.md for probe refresh/cached place readiness. No exact trigger/consequence/scope coverage or same-root
item found. Recipe-status and orphan-discovery issues use different readers and endpoints.","FILTER.md checked:
rare-glitch coverage is ambiguous, not established. The trigger is any overlapping cached request, with no proved narrow
duration; later automatic cached reads do not repair the missing probe. Retain rather than infer rarity or brief
self-correction."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Original
and latest records concern stale readiness and an ineffective explicit refresh; neither establishes security harm or
loss of user-owned work."; "No runtime ordering or frequency measurement. Those are not needed for the conditional
closure-loss conclusion; no stronger impact route is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f1`.
