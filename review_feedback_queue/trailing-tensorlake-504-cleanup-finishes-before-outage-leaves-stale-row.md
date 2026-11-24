# Deleted leftover stays listed if a 504 cleanup finishes before the follow-up discovery fails

Reviewed commit: de4302008ae668ca4ee7f569e7fad36e55521285

## TLDR

If you delete a remembered leftover sandbox while Tensorlake's sandbox listing is failing or timing out, and the
deletion takes just over 20 seconds (so Settings reports that cleanup carries on) and then succeeds before the follow-up
list check finishes failing, the already-deleted sandbox stays in the Settings list with an active Delete button until
listing recovers.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:344-358`.

`SPEC.md:610-611` (added in commit `de4302008ae668ca4ee7f569e7fad36e55521285`) requires: "A sandbox being cleaned up
appears once, with its cleanup state, and its row goes away when cleanup finishes even if discovery is still
unavailable." In `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:344-358`, `Sandboxes::delete` sends
`DELETE /api/tensorlake/orphans/{id}` through `operations::send`, which collapses any non-2xx reply into `Err(HostText)`
(`crates/gui/farhelm-gui/src/operations.rs:765-774`). When deletion exceeds `HOST_REPLY_BOUND` (20s),
`crates/farhelm-helm/src/tensorlake/orphans.rs:322-339` returns `504 Gateway Timeout` saying the cleanup carries on
while `delete_orphan` continues in the background. Because `Sandboxes::delete` receives `Err(reason)`, it does NOT
remove `id` from `self.last_discovered` (only `Ok(_)` does, `lines 348-351`), sets `self.delete_failure = Some(reason)`,
and immediately calls `list.read(cx)`. On the helm, `Tensorlake::orphan_sandboxes_by`
(`crates/farhelm-helm/src/tensorlake/orphans.rs:101-117`) runs `self.discover_orphan_sandboxes_by(reply_by)` first
(which waits up to 20s if `list_sandboxes` stalls) and calls `self.0.orphan_cleanups.snapshot()` only afterwards. If
`sweep_orphan` finishes any time between the 504 reply and that `snapshot()` call (up to a 20-second window during a
discovery timeout), `operations.remove(&row.id)` (`orphans.rs:244`) removes `id` before `snapshot()` runs. The follow-up
`GET /api/tensorlake/orphans` then returns
`OrphanSandboxList { discovery: OrphanDiscovery::Failed { .. }, cleanups: [] }`. Because the GUI never observed `id` in
any `cleanups` snapshot, `forget_discoveries_in_cleanup(&[])` (`tensorlake_sandboxes.rs:256-260`) leaves `id` in
`self.last_discovered`, so the deleted sandbox remains displayed with a Delete button (and the stale "the cleanup
carries on" notice beneath it) until discovery recovers. How to verify: in `tensorlake_sandbox_tests.rs`, populate
`last_discovered` with `sb-1`, trigger Delete on `sb-1`, reply 504 to `DELETE /api/tensorlake/orphans/sb-1`, then reply
`failed_discovery("provider unavailable", &[])` to the follow-up `GET /api/tensorlake/orphans`; `sb-1` remains painted
with a Delete button despite cleanup having finished. Fix: either return recently completed cleanup IDs (or a bounded
set of completed cleanup IDs on `OrphanSandboxList`) so the GUI retires them from `last_discovered` during an outage
even when cleanup started and finished between reads, or attach a `HelmMarker` to the post-admission 504 reply (after
bounding `delete_orphan`'s pre-admission local reads so 504 only fires once recorded) so `Sandboxes::delete` can retire
`id` from `last_discovered` once cleanup is confirmed recorded.

Reviewer metadata (g16_tensorlake_leftovers_discovery/l6): suggested bucket: other — stale leftover row and misleading
"cleanup carries on" message after cleanup already succeeded during a discovery outage; no security or data-loss risk
because the sandbox and its snapshots are already gone and clicking Delete again returns 404.; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g16_tensorlake_leftovers_discovery/l6`.
