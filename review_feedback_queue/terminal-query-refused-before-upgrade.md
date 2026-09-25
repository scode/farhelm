# Bad terminal size query is refused before the WebSocket upgrade

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A malformed terminal URL closes with no explanation instead of a readable reason.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F12 / COR-TERM-QUERY-400`, tagged **possible**. Anchors and title: `terminal.rs:10-27`, `terminal.rs:88-92`,
`terminal.rs:303-304` — A bad `?cols=`/`?rows=` is refused before the WebSocket upgrade, contrary to the module's "every
refusal on-socket" promise

The terminal WebSocket URL carries query parameters: initial `cols` and `rows`, plus optional `tab` and `lease` (the
client's attach identity). The module docs (lines 10–27) promise that once authentication has passed, every refusal,
explicitly including "an unparseable `?lease=`", is delivered on the open socket as a `{"type":"detached","reason":…}`
message followed by a close. The docs give the reason: a browser cannot read why a WebSocket handshake failed, so a
refusal before the upgrade looks like a terminal that flickered and vanished.

The handlers read the query with axum's `Query<TermQuery>` extractor (lines 303–304), and `TermQuery` declares `cols`
and `rows` as `u16` (lines 88–92). A non-numeric or out-of-range value (`?cols=abc`, `?rows=70000`) makes the extractor
fail before the handler runs, so axum answers with a plain HTTP 400 and the upgrade never happens. The browser sees
exactly the opaque failed handshake the module was designed to avoid.

The shipped UI always sends valid integers, so only hand-built URLs or a future client bug reach this. The effect is a
malformed terminal URL closing with no explanation, and the module's documented guarantee is false for this input class.

Suggested fix: accept the geometry as raw strings (or take `Result<Query<_>, _>`) and send parse failures through
`serve_term`'s notice-then-close path. The documentation-only alternative is to narrow the module docs.
