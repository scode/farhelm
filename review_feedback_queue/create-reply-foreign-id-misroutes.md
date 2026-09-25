# A create reply naming another host's session id routes to that host

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After creating a session on a misbehaving host, opening, typing into or stopping the "new" session can act on an
existing session on another machine for a few seconds.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F9 / COR-CREATE-REPLY-FOREIGN-ID`, tagged **possible**. Anchors and title: `sessions.rs:1274`, `sessions.rs:717`,
`sessions.rs:733`, `manager.rs:1908` — A create whose reply names an id another host already caches is routed to that
other host

`helm.db` allows at most one host to cache a given session id. Suppose a create on host B returns a session id that host
A already has in the cache. Honest supervisors mint UUIDs, so this takes a buggy or hostile B. Then:

1. The store refuses B's write-back with `SessionOwnerAmbiguous`. The `?` at `manager.rs:1908` propagates it, skipping
   any `refresh_now`.
2. `record_session` (`sessions.rs:1274`) only logs a warning, and the create returns success.
3. Nothing records the collision. `resolve_owner` (`sessions.rs:733`) only knows about **contested** ids, meaning ids
   that some host's _refresh_ reported while another host's cache held them. A rejected write-back does not add one.
4. `resolve_owner` therefore finds A as the cached owner with no contest, and `route_session` sends operations there.
   This continues until B's next refresh drains the id, marks it contested, and routing starts failing closed.

SPEC.md treats supervisor replies as untrusted, and SPEC_impl says routing fails closed on ambiguity. Here, the first
actions on the "new" session go to a different machine's session. Separately, `resolve_owner`'s docstring
(`sessions.rs:717`) cites `AppState::contested_sessions`, which does not exist anywhere in the crate. That half of the
fix is documentation only.

Suggested fix: when the write-back fails with `SessionOwnerAmbiguous`, mark B as a contested claimant for that id
immediately, or fail the create with a conflict naming both hosts. Call `refresh_now(B)` either way, and fix the
docstring.

User-visible consequence: after creating a session on a misbehaving host, opening, typing into or stopping the "new"
session can act on an existing session on another machine for a few seconds.
