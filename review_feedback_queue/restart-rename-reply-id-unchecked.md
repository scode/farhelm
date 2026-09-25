# Restart and rename replies are recorded without an id check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A misbehaving remote machine can answer a rename or restart with another session's details, so Farhelm shows or records
the wrong session and may make another session temporarily unreachable.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F29 / SEC-RESTART-RENAME-REPLY-ID`, tagged **possible**. Anchors and title: `sessions.rs:2614`, `sessions.rs:2662`,
`client.rs:2767`, `client.rs:2807` — Restart and rename replies are recorded without checking they describe the
requested session

`SupervisorClient::restart_session` and `rename_session` (`client.rs:2767`, `:2807`) return whatever `SessionInfo` the
supervisor's reply carries. `do_restart_session` and `do_rename_session` (`sessions.rs:2614`, `:2662`) write that reply
into the helm's view via `record_session` → `remember_session`, and return it to the caller, including agents through
the relay. Neither checks that `session.id` equals the id that was asked about. Create does validate its reply
(`created_session`), but these two paths do not.

For a host with an identity, the store's write-back refuses to overwrite another host's cached id, so the worst case
there is a planted row on the misbehaving host's own cache until its next refresh. For an identity-less host, the
in-memory insert skips that cross-host check entirely. A colliding id is only caught later, when `resolve_owner` refuses
to route it (the F28 effect).

The impact is limited: nothing gets misrouted, because the database refuses to overwrite and routing refuses rather than
guessing. It is still unvalidated peer input at a trust boundary. It lets a host plant ids outside the list and create
checks, and answer "X renamed" with Y's details.

Suggested fix: before `record_session`, refuse a reply whose id differs from the requested one as `SentInvalidReply`.

User-visible consequence: a misbehaving remote machine can answer a rename or restart with another session's details, so
Farhelm shows or records the wrong session and may make another session temporarily unreachable.
