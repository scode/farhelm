# Identity-less refresh publishes after releasing the cache lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a host whose supervisor reports no identity, a just-created session can briefly vanish and refuse to open, or a
just-deleted one briefly reappear, for up to one refresh interval.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F5 / COR-IDENTITYLESS-PUBLISH-OUTSIDE-LOCK`, tagged **definite**. Anchors and title: `manager.rs:3693`,
`manager.rs:3688`, `manager.rs:3481`, `manager.rs:1877` — An identity-less host's refresh publishes its list after
releasing the cache lock, so a concurrent create or delete can be overwritten

For identity-less hosts, the session list lives only in the actor's published status (`live_sessions`), not in
`helm.db`. `refresh_once`'s identity-less branch (`manager.rs:3682-3721`) does three things:

- takes `cache_lock` (`let _committing = self.cache_lock.lock().await` at :3693);
- checks that `seed_epoch` has not moved since the drain started;
- _returns_ `LiveSessions::Set(entries)` as data.

Returning drops the guard. The list is actually published later, back in `serve()`, by `publish_refresh` at
`manager.rs:3481`, with no lock held.

In that gap a write-back that was waiting on `cache_lock` can go through. That could be `remember_session` for a create,
restart or rename (`manager.rs:1877`), or `forget_session` for a delete. It checks the claim, inserts or removes the row
in `live_sessions`, and bumps `seed_epoch`. Then `serve()` publishes the refresh's older drained list over it, erasing
the write-back. This is exactly the lost update that `cache_lock` and `seed_epoch` exist to prevent. The comment at
:3688 says publication "is still ordered against seeds", but only the epoch check is ordered, not the publish. Hosts
with an identity are not affected, because their branch commits to the store while it still holds the lock.

Suggested fix: publish while the lock is held. Either return the guard inside `RefreshStep` and drop it after
`publish_refresh`, or do the `send_modify` inside `refresh_once` and return `Retain`.

User-visible consequence: on a host whose supervisor reports no identity, a just-created session can briefly vanish and
refuse to open, or a just-deleted one can briefly reappear, for up to one refresh interval.
