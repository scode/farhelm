# Orphaned connection actor drains refreshes with no backoff

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a host's connection handler is replaced, the orphaned one can hammer its machine with back-to-back refresh requests
until it is shut down, instead of waiting quietly.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

The connected-phase select's refresh arm is `_ = refresh.changed() => {}` (manager.rs:3432): `watch::changed()` resolves
immediately with `Err` once the sender is dropped, the arm matches `_`, and the loop goes straight into another
`refresh_once` drain. Every handle replacement (`sync_registry` respawn, `revive`, `stop_actor`, `shutdown`) drops the
old handle's sender while the old actor is still alive — abort is two-hop (supervisor → `AbortOnDrop` → actor) and needs
both tasks scheduled before it lands — so in that window the orphaned actor drains (`ListSessions` + profile resolve +
cache commit attempt) back-to-back with no sleep.

The nudge arm in the SAME select goes through `next_nudge` (manager.rs:2697), which deliberately `pending()`s on
sender-drop with the comment "the alternative (returning immediately, forever) would spin the loop hot in the window
before that abort lands" — the refresh arm is exactly the shape that comment warns against, fixed in one arm but not the
other. Extra supervisor round-trips and store writes per orphaned actor per abort-latency window; bounded by abort
delivery, not backoff; widens under CPU pressure. Possible, low.

Suggested fix: match on `refresh.changed()` and `pending()` when the sender is gone, like `next_nudge`.
