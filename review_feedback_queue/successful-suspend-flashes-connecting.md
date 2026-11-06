# Successful suspend flashes connecting

Reviewed commit: 627dc733a5fdaae9cd411554b15967ad4fdfc063

## TLDR

A confirmed sandbox suspension can briefly change back to Connecting during its status recheck, even though the sandbox
remains suspended.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/manager.rs:3909 at 627dc733a5fdaae9cd411554b15967ad4fdfc063`.

Source assessment (read with the later corrections below) (review record g015/l3, event 15):

Reviewed commit: 627dc733a5fdaae9cd411554b15967ad4fdfc063. TLDR: after confirming a sandbox has suspended, Farhelm can
briefly say it is connecting, confusing whether the pause completed. Details: tensorlake/lifecycle.rs:123-138 publishes
confirmed Suspended under the hold, then line 156 releases the hold with fresh_window=false on success.
manager.rs:3670-3684 retains the connection loop's active flag through parking (active |= request.fresh_window). If the
operation interrupted an active connection window, active remains true. connect_phase at lines 3906-3921 then publishes
Connecting before attempt at lines 4151-4177 queries Tensorlake and returns Suspended again. SPEC.md Topology lines
286-291 describes confirmed Suspend ending in Suspended, and SPEC_impl.md Tensorlake hosts describes publishing
confirmed suspension under the hold; neither calls for an attempted reconnection label after success. The assigned
report at plans/reports/tensorlake-ui-fixes.report.md:274-275 independently records the observed flash and says it
predates this plan. Suggested fix: preserve confirmed Suspended during the first background status probe after
successful Suspend, without weakening failure recovery or explicit Resume. Suggested bucket other, confidence confirmed
for the active-window route; exact UI duration is not measured in this source-only review. proposed_drop: false.
possible_cover: Tensorlake lifecycle implementation interface; parent checks overlap. Caveat: actor checks vendor state
before dialing, so this is inaccurate transient status, not proof that a suspended sandbox resumes or loses work.
Temporal qualification: pre-existing relative to the UI-fixes plan, not introduced by this planning-record commit; no
later manager.rs or lifecycle.rs edit occurs before pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g015/l3`; supporting records
`g044/l7`, `g049/iface_g015_l3`, `g020/d3`, `g034/c7`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
