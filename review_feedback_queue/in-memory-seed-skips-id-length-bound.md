# In-memory session seed skips the id length bound

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A session with an over-long id accepted from one host's reply can end up listed but permanently unopenable — every
attempt to address it is refused before it starts.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible. Coordinator verified; merge of two
reviewers' reports on the same lines.

`remember_session`'s `None` (identity-less host) arm (manager.rs:1849-1934) inserts the peer-supplied `session.id` into
the published `live_sessions` list checking incarnation, connectedness, and list presence — but never
`session.id.len()`. Both sibling paths refuse over-long ids: `drain_sessions` (manager.rs:767-773) and the durable path
(`HelmStore::remember_session`'s `anyhow::ensure!`, store.rs:4343-4346). The bound exists because an id past
`MAX_SESSION_ID_BYTES` (manager.rs:172-185) is embedded verbatim in REST paths, so it names a session no client can ever
address (the request head is refused first); the store's docs state a create's reply "is a peer ingress point exactly as
a drain's rows are" (store.rs:4320-4324) — yet this second ingress skips it.

Ingress is a mutation reply from the host's supervisor (on create, the id is peer-minted; caller `record_session`,
sessions.rs:848, passes the reply straight through; creates happen on identity-less hosts, manager.rs:1786-1788). The
stored row is served (merged list reads `live_sessions`; `live_owner` routes it). Count eviction bounds rows, not id
bytes, so each retained id can reach the wire frame size. Possible: needs a faulty/older peer minting an over-long id;
ordinary supervisors mint UUIDs.

Suggested fix: bail before the `send_modify` when `session.id.len() > MAX_SESSION_ID_BYTES` (or enforce above the
identity `match` so both shapes refuse identically), mirroring the store's ensure message — safe for the caller, which
already treats the error as a best-effort warning that self-heals at the next refresh.
