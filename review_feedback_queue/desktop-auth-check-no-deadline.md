# The desktop credential check has no deadline

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If the embedded helm stops answering at the wrong moment, the desktop window can sit on "Starting Farhelm…" forever
instead of showing an error.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F6 / COR-DESKTOP-AUTH-CHECK-NO-DEADLINE`, tagged **possible**. Anchors and title:
`crates/farhelm-ui/assets/desktop-auth.js:44` — The desktop credential check has no deadline, so startup can hang on
"Starting Farhelm…"

On every launch, and again on every re-authentication, `desktop-auth.js` first checks whether the webview's saved device
secret is still valid, with `GET /api/auth/device` (line 44). Unlike the other two network steps in the same script,
this request has no deadline: the token exchange (`exchange()`) uses an `AbortController` with a 5-second timer, and the
WebSocket check has a 5-second timeout. On the Rust side, `DesktopBootstrapGate` waits on `eval.recv()`, which also has
no deadline. If the embedded helm accepts the connection but never answers (a wedged handler, say), the window sits on
"Starting Farhelm…" indefinitely and shows no error. Native bootstrap puts a 30-second deadline on the equivalent
request, so this is the only untimed step in startup, and it runs on nearly every launch.

The suggested fix is to give this fetch the same abort-and-deadline pattern `exchange()` uses, and to report a timeout
as a visible validation error, which would then reach the failure paragraph (and, with F5 fixed, a retry).
