# Never-started connection handler logs a spurious row-gone warning

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Retrying a host at the wrong moment can log a scary warning that its entry is gone — for a host whose entry exists — and
wake every client to re-read nothing.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified. Found
independently by two lenses (correctness-general p2, correctness-state-lifecycle p3 — the latter adding the
`ActorHandle::start` doc contradiction below). Independent of `supervisor-discards-actor-panic-cause.md`: same `match`,
different arm.

`spawn_actor`'s gated future returns `Ok(())` when the gate sender is dropped without release — "it never runs at all,
and its task ends here" (manager.rs:1604-1608). The supervisor maps EVERY `Ok(())` to "the connection actor stopped
because its registry row is gone" (manager.rs:1627), then warns (:1634-1640), publishes `Retired` to the (detached)
status, and bumps `events` (:1662). It cannot distinguish "ran and retired" from "never started" — unlike the cancelled
arm (:1631), which correctly stays quiet. And `revive` drops replacements without installing them on two ORDINARY paths:
shutdown (:2493-2499, "dropped without ever being started") and lost arbitration (:2500-2510, "someone else already
revived it").

This contradicts the handle's own documented contract: "A handle dropped without releasing never runs its actor at all,
which is what makes losing a slot arbitration free of side effects" (manager.rs:962-963). The first half holds (the
gated task returns `Ok(())`), but the already-spawned supervisor's warn line and fleet bump ARE side effects of losing
the arbitration.

Realistic triggers, no exotic conjunction: double-click retry on a retired host, retry racing a host-mutation
`sync_registry` (which respawns dead actors first), or revive during shutdown. Impact: a WARN line in the reconnection
trail claiming the row is gone for a host whose row exists, plus a spurious fleet invalidation waking every client to
re-read nothing. Minor but observable and misleading. Possible, low.

Suggested fix: have the gated future report started-vs-never-started (bool/enum) and return quietly in the supervisor
when the gate was never released.
