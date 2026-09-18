# Editing a host mid-dial briefly routes operations to the old machine

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If you edit a host's destination while it is still connecting, operations can briefly go to the OLD machine — including
creating sessions on a machine the fleet no longer points at.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: definite. Coordinator verified; the area's
strongest finding.

`connect_phase`'s `select!` races dial completion against `next_nudge` with NO `biased` (manager.rs:3033-3038), so when
a retarget's nudge and the old dial's completion are both ready, either may win — and a won outcome is published
UNCONDITIONALLY by the outcome arms (manager.rs:2885-2951: Connected→`serve`, Skew/Mismatch/Unverified/Failed all
`set_state` with no `taken_nudge` check between `connect_phase` returning and publishing, unlike `serve`'s refresh path
which guards at :3392). `StaleAttempt` converts only handshakes completing AFTER the retarget's store commit; pre-commit
completions and all non-store-mediated outcomes (Skew, Unverified, transport Failed) publish stale. On the Connected
path it is worse: `serve` publishes `Connected` at entry (:3371) and runs a FULL `refresh_once` against the old client
before its first nudge check at :3392. (The Duplicate arms at :2840-2844/:2942-2944 are
`duplicate-freeze-clobbers-retarget-nudge.md`; this covers the sibling arms plus the Connected path.)

Why this is misrouting, not flicker: the retarget publish bumped the incarnation and took the client (None); the stale
publish re-supplies the OLD client, and `publish_refresh` mints a FRESH incarnation whenever the client changes
(manager.rs:3908-3916, None→old-client counts as a change). So for one refresh duration the new row is `Connected` to
the OLD endpoint with a fresh, VALID incarnation: operations route to the old machine and their seeds validate (claims
check incarnation only; no machine-identity re-check on the seed path) — brief misrouting with valid claims, including
creates whose sessions then live on a machine the fleet no longer points at. Self-corrects when the still-pending nudge
is consumed (next loop select takes the nudge arm and redials) — but that first post-entry select ALSO races both-ready
arms un-biased, so the stale window can be a full refresh duration. A stale `Mismatch` publish additionally offers an
adopt that then 409s via the store's `DialedAs` refusal. This breaks the documented invariant verbatim: "publishing the
state here rather than leaving it to the actor is what keeps `HostSnapshot`'s new-row/old-state pairing impossible"
(manager.rs:1371-1372) — the actor's stale publish re-creates exactly that pairing through the path the manager-side
publish was designed to close. Trigger is ordinary operation (retarget/row edit while a dial is in flight or a nudge is
pending) — no faulty peer needed. Definite.

Suggested fix: after `connect_phase` returns a settled (non-`Interrupted`) outcome, check `taken_nudge(nudge)`; on
`Some(n)` discard the outcome (drop the client on the Connected path) and `continue` with
`active = active || n.fresh_window`, mirroring the `Interrupted` arm and `serve`'s existing guard.
