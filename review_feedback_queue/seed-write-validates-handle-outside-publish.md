# Session seed validates the handle outside the publish

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In an extremely narrow race, a session you just created can briefly 404 — the create succeeded, but the session is
invisible until the next refresh.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified, scoped to
respawn.

In-memory `remember_session`/`forget_session` validate handle currency via `claim_is_current` (map lock, `ptr_eq` +
incarnation, manager.rs:2118-2128) at manager.rs:1821/2006, but the subsequent `status.send_modify` runs on the cloned
`Arc` with no lock held. A `sync_registry` respawn/revive replacing the entry on another thread in between redirects the
write to a detached sender ("stays perfectly usable", per the comment at :1814-1820 that motivated the re-read): the
create already succeeded on the supervisor, but the session is invisible to `live_owner`/merged list until the next
refresh — a just-created session 404s transiently (`changed`/`events.bump` fire pointlessly).

Scoped precisely: same-sender retarget (which bumps the incarnation on the live sender, :1483-1503) IS caught by the
incarnation re-check inside the modify — only entry REPLACEMENT (new sender, old incarnation value intact on the
detached one) escapes, in a span of a few synchronous instructions with no await. The durable path's store identity
binding covers its longer window; the in-memory path has no second defense. Vanishingly narrow + transient +
self-healing, hence low. Possible, low.

Suggested fix: hold the map lock across check+publish (both sync), or re-validate `ptr_eq` after the modify and bail.
