# Refresh timeout misses the profile and commit tail

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the local store stalls, a host can read as healthily connected while answering nothing — the refresh deadline does
not cover the stages that follow the data fetch.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

`refresh_once` wraps ONLY `drain_sessions(client)` in `refresh_timeout` (manager.rs:3513). What follows is unbounded:
`resolve_session_profiles_from_store(...).await` (:3578), then — under `_committing = cache_lock.lock()` (:3631) —
`store.replace_host_sessions(...).await` (:3655-3656). Meanwhile `serve`'s select (:3381-3386) races only `next_nudge`
against `refresh_once`; `client.closed()` is NOT polled during a refresh. So if the store stalls (disk stall, lock
contention, wedged SQLite — peer-side wedges ARE covered by the drain timeout), this host's actor sits in `refresh_once`
past any deadline: no refresh completes, a dead connection is not noticed, and the published state goes
stale-`Connected`.

The `REFRESH_TIMEOUT` docs (:187-201) state the timeout exists precisely so nothing can "park `serve` forever" ("no
refresh completes, no loss is detected, and the host reads as healthily connected while it answers nothing") — the exact
symptom a store stall reproduces — and `refresh_once`'s docs (:3501-3504) claim "the walk is bounded by
`refresh_timeout`", true of the drain but not the two store awaits after it. The stall also propagates: `cache_lock`
held across the commit blocks `remember_session`/`forget_session` and `host_write_lock` holders (retarget, alias,
remove) for that host. Note the asymmetry marking this as oversight rather than design: the connect path's store write
(`record_first_contact` at :3267) IS inside `deadlined_attempt`'s timeout (:3071). Recovery via `retry_now` nudge still
works (cancellation honored); reads as stale-connected with frozen cache until then. Requires store pathology; contained
to one host. Possible, low.

Suggested fix: extend the timeout to cover the post-drain stages (wrap the resolve-plus-commit tail, or the whole
post-epoch body, in the same `refresh_timeout`), treating expiry like drain expiry (end the connection, keep the
previous cache) — no new hazard class, since task abort can already drop the commit future at any await and every such
store call is transactional per the module docs.
