# A keyed spawn can hand the asker its own id as the new child

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent re-running a keyed spawn can be told it created itself, and its clean-up then stops or restarts the agent.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F6 / COR-SPAWN-SELF-REPLAY`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:3180-3197`, `farhelm-supervisor/src/service/handlers.rs:3309-3337`,
`farhelm-supervisor/src/service/handlers.rs:795-825`, `farhelm-supervisor/src/service/core.rs:7848-7890`,
`farhelm/src/main.rs:710` — A keyed `farhelm spawn` without `--parent` can hand the asking session its own id as its new
child

This is the same shape as F5, on the host-local `farhelm spawn` path. A spawn's idempotency key is scoped to the host,
not to the asker. It stays reserved as long as the child exists (`CreateAdmission::Spawn` maps to
`DedupScope::SessionLifetime`, `handlers.rs:62`). The replay match is again a fingerprint of cwd, launch bundle, title,
and optional parent (`handlers.rs:795-825`). The authenticated asker is not part of it. `handle_restricted_control`
knows the asker (`auth.session_id`, `handlers.rs:3309-3337`), and it checks the asker only against an explicit
`--parent`, which may name only the asker (`handlers.rs:3180-3197`). When the key hits a reservation,
`resolve_reservation` (`core.rs:7848-7890`) checks the fingerprint and returns the existing session. It never checks
whether that session is the one asking.

A concrete sequence:

1. P runs `farhelm spawn --cwd D --inherit-agent --idempotency-key K`, with no `--parent`, and gets C.
2. `--inherit-agent` copies P's stored launch bundle into C, so C's bundle is identical to P's.
3. If C later runs the same command, its request fingerprints identically. The key K is still live because C exists, so
   the supervisor replays C.
4. C's spawn prints C's own id as "the child session id" (`main.rs:710`).

The same holds for any other session on the host that reuses K: it is handed a child it did not create. The reviewers
also suggested a helm-made create with key K, followed later by a same-cwd, same-profile spawn under K. I did not
confirm that the two paths produce byte-identical fingerprints.

SPEC's spawn contract promises that stdout is the child id, and agents go on to target it with
`farhelm agent
stop`/`restart`, which here would hit the agent itself. `--parent` is optional and absent by default, so
the one field that could tell the requests apart usually is not there. The helm's clone path already refuses the
equivalent (see F5). The open premise is how realistic key reuse between a parent and its child is. The suggested fix is
to refuse, on a session-authenticated create, any replay whose id equals `auth.session_id`, with a `Conflict` asking for
a fresh key. Consider also folding the asker into the spawn fingerprint as an implied parent, so that a key belongs to
one asker.
