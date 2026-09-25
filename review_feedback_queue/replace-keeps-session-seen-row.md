# Replace leaves the source session's session_seen row

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None visible; each replaced session leaves a few bytes of orphaned read/unread bookkeeping.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F21 / COR-REPLACE-KEEPS-SEEN`, tagged **definite**. Anchors and title: `sessions.rs:3232`, `sessions.rs:3276` — Replace
deletes the source session without dropping its session_seen row

The helm keeps per-session read/unread state in a `session_seen` table keyed only by session id, with no foreign key.
SPEC_impl (around line 2131) says: "Deleting a session drops its `session_seen` row explicitly". It accepts leftovers
only for sessions deleted through another helm or dropped because their host was removed.

`delete_session` follows this: on success it calls `state.store.clear_seen(&id)` (best effort), then `forget_session`.
**Replace** creates a new session and then deletes the old one. Its tail, `finish_replacement` (`sessions.rs:3224`),
deletes the source on the supervisor and calls `forget_session` (`sessions.rs:3276`), but never calls `clear_seen`.

Each replaced session therefore leaves a small orphaned row. This is a deviation from a stated cleanup contract, not a
user-facing bug. The garbage is unbounded over time but tiny per row.

Suggested fix: after the source delete succeeds in `finish_replacement`, call `state.store.clear_seen(id)` best-effort,
logging any failure as `delete_session` does.

User-visible consequence: none visible. Each replaced session leaves a few bytes of orphaned read/unread bookkeeping.
