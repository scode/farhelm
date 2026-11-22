# Restart with samples terminal availability before its final database work

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

Restart with can return and cache a terminal-present acknowledgement after the replacement terminal is gone.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:12880`.

host_restart.rs:388-391 calls restart_reply after old-host cleanup. Core.rs:13181-13200 samples terminal_missing after
checkout metadata. For a published Restart with, core.rs:12880-12909 then awaits update_launch and the sessions lock,
and :12919-12920 returns the earlier availability bit. A replacement that becomes Gone in that window yields a false
bit. The helm records the reply at crates/farhelm-helm/src/sessions.rs:3592-3604, and session_cache.rs:162-172 keeps the
reply's bit even when a listing already disagrees. SPEC_impl.md:8379-8383 describes the reply as evidence of the new
terminal and SPEC.md:2344-2347 ties the missing bit to recovery. Refresh the bit at the final supervisor reply boundary
after persisting Restart with settings. Bucket: other. The established effect is stale reply and cache during a narrow
window. The desktop launcher does not directly dismiss the card from Restart with's reply (launcher/launch.rs:668-682).
The timing or duration of any UI effect was not established. Confidence: confirmed by source for ordering and cache
behavior, no runtime observation. Possible coverage: recovery_contract. Synthesis with recovery_contract/l15: at
44137d0659d28b9cadf90a8ffe96a65930035aad core.rs:13137-13155 samples the tab vector before relaunch and :13276-13278
copies it into the reply. Tab liveness can change independently of the replacement main terminal, even during plain
Restart. Keep l15 as separate reply metadata: refresh from the preserved tab cell at publication or at the reply
boundary. For r8 specifically, the terminal bit must be sampled after the Restart with settings write and final
sessions-map wait. Refreshing tabs earlier does not cover that later window. A shared final sampling site could handle
both fields, with separate assertions and no assertion that stale tab metadata hides the recovery card. Cache interface
confirmation for l15: at 063203912059cb23101d9a458d8821f02f1bc18f sessions.rs:1754-1763 passes the entire SessionInfo to
the cache, manager.rs:2582-2611 inserts the merged value in memory, and store.rs:9012-9039 serializes the merged value
in the durable cache. session_cache.rs:187-192 merges only status and the activity timestamps, so stale tabs as well as
the terminal bit can enter the cache. This establishes stale tab metadata in the cache, with no assertion of a visible
or actionable tab in the UI.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r8`.
