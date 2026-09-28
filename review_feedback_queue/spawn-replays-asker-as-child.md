# A reused spawn key hands another session a child it did not create

Reviewed commit: 7cc06814956a1e9b6ec41f29e2e57ec57b5d2178

## TLDR

A session that re-runs a keyed `farhelm spawn` whose key another session on the same host already used, with the same
directory, launch bundle and title, is told the other session's child is its new child. Its later stop or restart of
"its child" then acts on a session it never created.

## Details

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0349-2b597e9-145b`, `F6 / COR-SPAWN-SELF-REPLAY`).
The self-replay case, where the replayed session IS the asker, is now refused with `Conflict` in `handle_create_session`
(`crates/farhelm-supervisor/src/service/handlers.rs`, the `Ok(session)` arm guarded on `restricted_auth`). What remains
is sibling key reuse.

A spawn's idempotency key is scoped to the host (`CreateAdmission::Spawn` → `DedupScope::SessionLifetime`), and the
replay match is a fingerprint of cwd, launch bundle, title and optional parent (`create_fingerprint`). The authenticated
asker is not part of it, and `--parent` is optional and absent by default. So session A's keyed spawn creates C; session
B later runs the same command with the same key while C exists, and `resolve_reservation`
(`crates/farhelm-supervisor/src/service/core.rs`) replays C to B. SPEC's spawn contract promises stdout is the new
child's id.

The open premise is how realistic key reuse across sessions is: keys are caller-chosen, and two agents choosing the same
key for the same launch is unusual. The suggested fix is to fold the asker into the spawn fingerprint as an implied
parent, so a key belongs to one asker. That changes a durable fingerprint encoding, so it needs a versioned discriminant
like the fresh-checkout one rather than an in-place edit.
