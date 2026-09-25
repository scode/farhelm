# Event-feed cap refusal is an HTTP 503 browsers cannot see

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When the feed is full, users get no indication why live updates degraded; it looks like a connectivity problem.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F7 / COR-EVENTS-503`, tagged **definite**. Anchors and title: `events.rs:129-133`, `events.rs:148-157` — The
subscriber-cap refusal is an HTTP 503 before the upgrade, which browsers cannot see

When all 64 event-feed seats are taken, `events_ws` does not upgrade the connection to a WebSocket. It returns HTTP 503
with a prose body ("this helm is already serving its maximum of 64 event subscriptions; retry shortly"; lines 148–157).
Its docstring (lines 129–133) gives the reason: "an HTTP status is something the client can see and back off from",
unlike a socket that opens and immediately closes.

That premise is wrong for browsers. The WebSocket API used by the web UI and the desktop webview (`assets/events.js`)
does not expose the status code or body of a failed handshake. The page sees an `error` event followed by a `close` with
code 1006, exactly what a network failure or a stopped helm produces, and it follows its normal reconnect ladder. The
terminal socket's own module docs (`terminal.rs` lines 22–25) say the same thing, which is why terminal refusals are
delivered after the upgrade.

In practice, cap exhaustion (for example from F6's zombie seats) cannot be told apart from the helm being down, and the
user gets no hint about why live updates stopped. Future code that trusts the docstring would be building on a signal no
client can observe.

Suggested fix: accept the upgrade and close immediately with an application close code (4000–4999) and a reason string,
which the browser does expose. The documentation-only alternative is to correct the docstring.
