# A committed cache change is not announced when a nudge ends serve

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a host retarget or retry, other open clients briefly show an out-of-date session list.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F16 / COR-NUDGE-SKIPS-CACHE-BUMP`, tagged **possible**. Anchors and title: `manager.rs:3475` — A committed cache change
is never announced when a nudge ends the connection

Clients learn about changes through a **fleet revision** counter, which they watch and re-read on (`events.bump()`).
After each refresh, `serve()` bumps it if the cache changed. But the check at `manager.rs:3475` comes first: if a nudge
(retarget or retry) arrived while the refresh was finishing, the loop breaks before the
`if cache_changed { events.bump() }` line. A refresh that had already committed a real change to the cache is then never
announced.

The final `Connecting` publish on the way out often does not cover it either. After a retarget, `sync_registry` has
already set the state to `Connecting` (and bumped then, _before_ the commit landed), so the actor's own `Connecting`
publish compares equal and bumps nothing. After a plain Retry the state does change from `Connected` to `Connecting`, so
that case usually does get a bump. The staleness lasts until the next bump, which in practice is usually the reconnect
publishing `Connected` shortly afterwards.

Suggested fix: bump for `cache_changed` before breaking out on the nudge.

User-visible consequence: after a host retarget or retry, other open clients briefly show an out-of-date session list.
