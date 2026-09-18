# Disconnect keeps stale claims, spuriously refusing session actions

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After a host disconnects or is retargeted, actions on its former sessions can keep being refused as "ambiguous" because
of a collision with a machine that is no longer connected at all.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible. Coordinator verified; three reviewers
independently reported the same lines.

Two direct disconnect publishes withdraw the client and bump the incarnation but leave `status.contested` untouched:
retarget in `sync_registry` (manager.rs:1483-1503: takes client, clears `live_sessions`, bumps incarnation — no
contested clear) and supervisor retire (manager.rs:1647-1656, same shape). The module's own rule (manager.rs:3917-3926)
is explicit — "a host that is no longer connected takes its claims with it" — `publish_refresh`'s `(None, false)` arm
implements it, and `adopt` (manager.rs:2338) clears explicitly; these two are the only disconnect paths that don't.

The stale ids keep flowing to `contested_claimants` (manager.rs:2198-2215), whose only non-test consumer is
`resolve_owner`, which refuses the session as `SessionOwnerAmbiguous`. Fail-closed (spurious refusals, never
misrouting), but the retire window lasts until a retry or reconcile respawns the actor, and the error names a retired
host for a collision with no live evidence behind it; the retarget window is momentary (the actor's next publish clears
it). Independent of `resolve-owner-compares-first-claimant-only.md` (that is about masking live disagreement; this is
about spurious refusal from dead hosts). Possible.

Suggested fix: add `status.contested = Arc::new(Vec::new());` in both `send_modify` blocks, matching `adopt`.
