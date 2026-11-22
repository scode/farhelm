# Restart can republish a tab that exited during relaunch

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

A tab that exits during a slow Restart can still appear in the Restart result and be published by the helm before a
fresh listing corrects it.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:13276`.

#4529 replaces the obsolete post-discovery wording at core.rs:13276–13278 with an explicit pre-restart tab snapshot. The
implementation captures tabs before awaiting the new host (13137–13155) and sends them in SessionRestarted
(handlers.rs:2234). SPEC_impl.md:2035–2049 says independently exited tabs are hidden at once; SPEC.md:2105–2106 requires
Restart to leave open tabs running. Helm sessions.rs:3428,990–994 forwards the reply as JSON and 3602–3604 calls
record_session. manager.rs:2542–2554,2582–2611 and store.rs:9023–9040 store the vector on a current claim, and
session_cache.rs:187–193 does not merge tabs with fresher evidence. Direct GPUI Restart completion uses only
terminal_missing (session_list/restart.rs:307–315). GPUI terminal/set.rs:532 derives tab IDs from the listed row and
session_list/stop.rs:72,80,99 uses the listed tabs for the Stop guard and warning. Source establishes stale shipped data
and the listing's consumers. A particular stale draw/action still depends on the observed refresh and UI-feed order, so
the finding claims no destructive consequence. Refresh the reply-facing tab list at publication. Bucket: other.
Confidence: confirmed by pinned source, no runtime observation. Possible coverage: recovery and lifecycle_unit
interfaces.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l15`.
