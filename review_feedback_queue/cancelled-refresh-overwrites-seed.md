# A cancelled refresh still commits over a just-seeded session

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a rare race with a host retry, a session created at that moment can briefly disappear and answer "no such session"
until the next refresh.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F6 / COR-CANCELLED-REFRESH-OVERWRITES-SEED`, tagged **possible**. Anchors and title: `manager.rs:3465-3469`,
`manager.rs:3727-3752`, `store.rs:4349` — A refresh cancelled by a nudge still commits its pre-create snapshot after a
seed landed

`serve()` races each refresh against a **nudge**, a manager request to reconnect, sent by a retarget or a Retry click
(`manager.rs:3465-3469`). If the nudge wins, the `refresh_once` future is dropped. For a host with an identity, the
refresh may already have taken `cache_lock`, passed its epoch check and called `store.replace_host_sessions`
(`manager.rs:3727-3752`). That store call runs its SQLite transaction inside `spawn_blocking` (`store.rs:4349` onward),
so dropping the future releases `cache_lock` but does not stop the blocking task: the stale replacement still runs and
commits.

For a plain Retry (`retry_now`), the nudge does not change the host's connection **incarnation**, the token the manager
uses to tell whether a write-back still belongs to the current connection. The incarnation changes only when `serve()`
later publishes the disconnect. So a write-back waiting on `cache_lock` can take it in the gap and pass
`claim_is_current`. Its own `spawn_blocking` can also win the database mutex before the refresh's blocking task does.
The seed then commits first and is erased by the pre-create snapshot committing after it. (A retarget does bump the
incarnation in `sync_registry` before nudging, so the seed would be refused and this race needs a plain Retry.)

This is another path to the lost update that `seed_epoch` guards against, but it needs a narrow three-way timing
coincidence.

Suggested fix: have one spawned task own the guard, the epoch check and the write together, so cancelling the caller
cannot separate them.

User-visible consequence: in a rare race with a host retry, a session created at that moment can briefly disappear and
answer "no such session" until the host reconnects and refreshes.
