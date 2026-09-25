# Create on a just-connected identity-less host is unroutable

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a host whose supervisor reports no identity, a session created right after the host connects answers "no such
session" to open/stop/rename until the host's list refreshes successfully.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F8 / COR-IDENTITYLESS-CREATE-UNROUTABLE`, tagged **possible**. Anchors and title: `manager.rs:1930`, `manager.rs:3454`,
`manager.rs:3714` — A session created on a just-connected identity-less host is not recorded and cannot be routed until
the first successful refresh

When a connection comes up, `serve()` publishes `Connected` through `self.publish(...)` (`manager.rs:3454`). That call
defaults the in-memory list to `LiveSessions::Clear`, which sets `live_sessions = None`. The list becomes `Some(...)`
only when a refresh succeeds and returns `LiveSessions::Set` (`manager.rs:3714`).

The create path accepts a host as soon as it is `Connected`, so a create in that window succeeds on the supervisor. The
write-back then fails: `remember_session`'s in-memory branch treats `live_sessions == None` as "this host no longer
serves from memory" and refuses (`manager.rs:1930`). `record_session` only logs the refusal. `live_owner` cannot find
the new session, and nothing is cached for an identity-less host, so `route_session` and `get_session` answer 404.

The window normally lasts for the first drain (seconds). If the drain that was already running started before the
create, it lasts until the next one. If refreshes keep failing, each failed refresh returns `Retain` and `live_sessions`
stays `None` indefinitely. The manager's own docs (`manager.rs:1840-1841`) say that "every operation on a just-created
session 404'd" is exactly what the in-memory path was added to fix. Identity-less supervisors are uncommon.

Suggested fix: publish `LiveSessions::Set(empty)` when an identity-less host connects. Alternatively, have
`remember_session` treat `None` as an empty list when the claim is current, the host is connected and it has no
identity.

User-visible consequence: on a host whose supervisor reports no identity, a session created right after the host
connects answers "no such session" to open, stop and rename until the host's list refreshes successfully.
