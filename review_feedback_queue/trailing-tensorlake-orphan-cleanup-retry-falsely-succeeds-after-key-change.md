# Retrying an unfinished orphan cleanup after switching Tensorlake API keys falsely succeeds via 404

Reviewed commit: de4302008ae668ca4ee7f569e7fad36e55521285

## TLDR

If a leftover sandbox cleanup failed (for example, snapshot deletion failed) and you then switch Settings to a different
Tensorlake account's API key, the old account's unfinished cleanup stays in Settings; clicking Retry cleanup appears to
succeed and removes the row, even though the old account's snapshots were never deleted and may keep billing.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/tensorlake/orphans.rs:178-283`.

In `crates/farhelm-helm/src/tensorlake/orphans.rs:42`, `Cleanups(Mutex<HashMap<String, OrphanSandbox>>)` stores
unfinished explicit deletions keyed only by canonical sandbox ID, with no API-key/account scope, and
`PUT /api/tensorlake` (`crates/farhelm-helm/src/tensorlake/mod.rs:462-478`) never clears or invalidates
`orphan_cleanups` when the API key is replaced or cleared. In `de430200`, `orphan_sandboxes_by` (`orphans.rs:101-117`)
always returns `self.0.orphan_cleanups.snapshot()` in `OrphanSandboxList.cleanups` regardless of the active key (so even
after Clear empties `Sandboxes.cleanups` locally in `tensorlake_sandboxes.rs:169`, saving a new account's key
immediately repopulates the old account's unfinished cleanup rows). If the user then clicks Retry cleanup under the new
account's key: (1) `delete_orphan` (`orphans.rs:179-189`) loads the new key via `self.api_key()`, finds
`previous = Some(row)` in `orphan_cleanups`, and skips `get_sandbox`; (2) `sweep_orphan` (`orphans.rs:260-283`) calls
`terminate_sandbox(&new_key, id)` and `wait_until_terminated` (`hosts.rs:680`), both of which treat `404 NotFound` from
the new account as `Ok(())`; (3) `list_snapshots(&new_key)` returns only the new account's snapshots, finds zero
matching `snapshot.sandbox_id == Some(id)`, and returns `Ok(())`; (4) `delete_orphan` removes `row.id` from
`orphan_cleanups` (`orphans.rs:242-245`) and returns `204 No Content`. Note: `Cleanups` / `delete_orphan` /
`sweep_orphan` and `put()` are pre-existing outside `de430200`'s diff, while `de430200`'s `OrphanSandboxList.cleanups`
split and key-clear handling directly surface the cross-key cleanup list. How to verify: in `orphans.rs` tests, fail
`delete_snapshot` for sandbox `sb-1` under key A, replace the stored key with key B (different fake account/state), list
orphans (sees `sb-1` in `cleanups`), call `delete_orphan("sb-1")`, and observe `Ok(())` and empty `cleanups` while key
A's snapshot remains. Fix: associate each entry in `Cleanups` with a hash/digest of the API key that admitted it (or
reject/clear mismatched-key entries on key replacement or retry, refusing retry with an explanation that the saved API
key changed rather than treating another account's 404 as cleanup completion).

Reviewer metadata (g16_tensorlake_leftovers_discovery/l7): suggested bucket: other — cleanup/correctness bug that
silently discards retry state for unfinished cloud snapshots on an old account after switching API keys (risking
continued billing on the old account), without security compromise or loss of live user-owned data/work.; confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g16_tensorlake_leftovers_discovery/l7`.
