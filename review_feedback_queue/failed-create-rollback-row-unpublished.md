# A failed create rollback leaves an invisible, undeletable row

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A failed create can leave a hidden session that can't be seen or deleted until the supervisor restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F7 / COR-FAILED-ROLLBACK-UNPUBLISHED`, tagged **possible**. Anchors and title: `service/core.rs:12465-12490`,
`service/core.rs:8647`, `service/core.rs:9218`, `service/core.rs:9297`, `service/core.rs:9520`, `store.rs:5331` — When a
failed create's rollback fails, the kept row is never published, so it can't be listed or deleted until restart

The supervisor keeps two views of each session: a durable row in SQLite, and an in-memory map entry. Listing and every
lifecycle operation (Stop, Delete, Rename, Restart) look sessions up in the in-memory map. A session whose row exists
but has no map entry therefore cannot be seen or managed until the next supervisor start, when reload rebuilds the map
from the database. The create code has a stated rule for this: `publish_retained_launch`'s docs say "every create exit
that retains such a row publishes through this helper".

When a create's launch provably started nothing, `launch_reserved` rolls back through `abandon_launching_record`
(core.rs:12465-12490). "Provably" covers three cases: the launch spec file could not be written (core.rs:9218), tmux
confirmed the session does not exist (core.rs:9297), or the terminal was killed after confirmation failed
(core.rs:9520). The rollback deletes the launching row and, in the same transaction, records the create's idempotency
key (the "intent key" a client sends so a retried create is recognized) as failed. If that transaction fails, the helper
only adds a note to the error: the record "will list as unknown until it is deleted". Nothing puts the kept row into the
map.

On a first attempt the entry was never published. On a keyed retry, the retry's takeover step already removed the entry
from the map (core.rs:8647). Either way the row is invisible, and Stop/Delete/Rename/Restart answer `NotFound` until the
supervisor restarts. The error text promises the opposite. The sibling paths that keep a row in the same way
(`retain_create_refusal`, `abandon_fresh_pre_mkdir`) do publish a fallback entry.

This only happens if `delete_session` fails during rollback. Besides a SQLite error, it deliberately refuses when the
session holds the last reference to a managed checkout that is not retired (store.rs:5331). The fix is to publish the
retained row when the delete fails. The caller already holds the snapshot, so `publish_retained_launch` with
`LastOutcome::Launching`, or `publish_retained_stored_session`, would do. What the user sees: after a rare rollback
failure, a failed create leaves a hidden session that cannot be seen or deleted until the supervisor restarts.

Restater note: The fresh-checkout case, where the last-reference refusal would trigger every time, is routed to
`retain_create_refusal` before `abandon_launching_record` is reached (core.rs:9204 and around 9290). That leaves the
refusal reachable only when a create borrows an existing non-retired checkout that has no other member. I did not
establish how often that happens, so the trigger is probably a rare SQLite failure.
