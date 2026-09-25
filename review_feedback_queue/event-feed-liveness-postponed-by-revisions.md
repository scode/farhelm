# Event-feed liveness check is postponed while revisions flow

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a busy fleet reached through SSH forwards, dead connections pile up until new tabs (or the desktop app) are refused
live updates ("maximum of 64 event subscriptions").

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F6 / COR-EVENTS-KEEPALIVE`, tagged **definite**. Anchors and title: `events.rs:218-245`, `events.rs:52-56`,
`events.rs:106-114` — The event feed's liveness check is postponed indefinitely while revisions keep flowing, so dead
subscribers hold seats

`/api/events` is a WebSocket that pushes a revision number whenever anything in the fleet changes, so each UI tab knows
when to re-read. The helm allows at most 64 subscribers at a time (`MAX_SUBSCRIBERS`). A peer that vanishes without
closing its socket, such as a laptop that went to sleep, would hold its seat forever, so `serve_events` runs a
keepalive. After 30 idle seconds (`IDLE_PING_INTERVAL`, lines 106–114) it sends a WebSocket Ping and sets
`awaiting_liveness`. If the next idle deadline passes without any inbound frame, it drops the subscriber. The module
docs (lines 52–56) promise that a vanished subscriber costs "one idle interval, at most two in the boundary case".

The bug is in the loop (lines 218–245). The same `idle` timer is also reset after every successful revision write,
including while `awaiting_liveness` is set. On a fleet where something changes more often than every 30 seconds, each
revision pushes the deadline back another 30 seconds, and the unanswered Ping is never judged. The constant's own
docstring says a revision write "resets this idle window, but only a Pong … answers the Ping", yet the reset is what
stops the answer from ever being checked.

A write counts as successful whenever the transport accepts the bytes, and a dead peer's transport often still accepts
them. With an SSH `-L` port forward (SPEC's way to reach the UI from another machine), the helm's TCP peer is the local
`sshd`. After the remote laptop sleeps, `sshd` keeps accepting small revision messages until its SSH channel window
fills, which at a few dozen bytes per revision takes a very long time. Only then does the 10-second write deadline
notice anything.

On a busy fleet reached through SSH forwards, these dead seats pile up until the 64-seat cap is reached. New tabs are
then refused live updates (see F7 for how that refusal looks).

Suggested fix: keep a separate liveness deadline that is set when the Ping is sent and cleared only by an inbound frame.
The simpler alternative is to skip the `idle` reset after a revision write while `awaiting_liveness` is true.

Restater note: the finding also names the desktop webview's startup check (`crates/farhelm-ui/assets/desktop-auth.js`
opens `/api/events` and treats a refusal as a failed startup) as a victim. That is accurate, but it only applies if the
desktop app's own embedded helm is the one whose seats fill up. That helm is normally reached only by its own webview,
not through SSH forwards, so the desktop impact needs someone to be forwarding the desktop helm's port as well.
