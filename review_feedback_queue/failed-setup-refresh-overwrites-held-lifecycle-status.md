# Failed setup refresh overwrites held lifecycle status

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

After setup's registry synchronization fails, a later refresh can replace in-progress Suspend or Resume with Connecting
while the operation is still running.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/manager.rs:1776-1829 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g034/l10, event 28):

Reviewed commit 1a57aa8e265cbb59cedc228433b189f59f7ad59a; checked relevant source at origin
8a7f66ede90651d47d1facc1e877ce81f842af1a and pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. New discovery pass 3
source finding. TLDR: if sandbox setup saves install paths but fails its following registry synchronization, a later
global registry refresh can change Suspending/Resuming to Connecting while that lifecycle request is still running. The
connection remains parked; no premature dial, competing vendor operation or work loss is asserted. Producer:
provisioning/tensorlake_add.rs:486-496 at reviewed/tip admits durable install coordinates then fallibly synchronizes.
Failure cleanup :360-381 releases the setup hold once the coordinates exist and records failed progress; the owned run
guard drops when the task ends (:340-346). It does not successfully reconcile the stale manager row before releasing the
guard. Thus manager handle.row can still carry absent/old coordinates. Actor row reload does not repair handle.row:
spawn_actor explicitly passes actor.run its own row clone (manager.rs:1975-1983), while the map's cached row is
independently updated by sync_registry. Start a later allowed lifecycle operation on this failed-setup host and keep its
API request pending. A global reconcile caused by another host's alias/edit/add request then reads the durable
coordinates; reconfigured :1433-1437 detects destination/executable/state-dir differences, and :1806-1825 writes
Connecting without checking status.hold. It updates handle.row (:1794) and nudges the actor (:1827-1829), but the parked
actor still cannot publish while held (:5089-5093). For an ordinary real producer, another host's alias update calls
global sync_registry at hosts.rs:1108-1118; it owns the other host's lock, not this sandbox's lifecycle lock. SPEC.md
Topology :286-291 requires Suspending/Resuming until cloud confirmation and no competing lifecycle controls. Those
controls are disabled by displayed phase, so erased progress can reoffer controls that the server still refuses as busy.
Ordinary successful transport edits were investigated and excluded: hosts.rs:1012-1042 retains provisioning lock through
convergence, setup owns its guard, and sync_registry serializes read/publication end-to-end (:1679-1682). This finding
specifically needs failed post-admission convergence, not an unfenced normal edit. Temporal: the same save-then-sync and
release-on-failure order exists at 8a7f66ede90651d47d1facc1e877ce81f842af1a tensorlake_add.rs:350-374,480-495; manager's
unguarded reconfiguration publication is there at :1792-1805. The lifecycle operation added at that commit is already
vulnerable to held-state overwrite; reviewed #3048 adds explicit TensorlakeChanging intent, making the reported
in-progress display consequence precise. Later path log contains c340166d754d3edb3de7fd538c6408948d3536c6 and
1a57aa8e265cbb59cedc228433b189f59f7ad59a only; the checked save/sync/failure-cleanup/reconfiguration branches survive at
tip. Suggested bucket other, confidence confirmed source-admitted interleaving; no runtime frequency/duration
measurement. Regression: fail/gate the post-admission registry sync, await failed setup and free provisioning lock,
begin a gated Suspend, trigger another host's successful global reconciliation, assert held lifecycle phase persists
until completion. Preserve held display in sync_registry while still updating coordinates and invalidating stale
clients/claims. proposed_drop none. possible_cover g044:l11 is the same broad manager-publication/hold invariant but a
different exact locus (adopt :2732) and producer (concurrent identity adoption). Correcting adopt alone does not fix
sync_registry :1806; parent may consolidate both under the invariant with both loci/producers retained. This is not
g015:l3 active-window flash after successful Suspend, g035:l4 retired-task failure or g034:l2 ambiguous Resume. Caveats
requires a transient post-admission registry-sync failure and a later overlapping global reconciliation; failures before
admitting coordinates or fully successful setup are not this route. All checks source-only; no
runtime/build/process/cloud/live-install action.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g034/l10`; supporting records
`g044/l14`, `g049/iface_g034_l10`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
