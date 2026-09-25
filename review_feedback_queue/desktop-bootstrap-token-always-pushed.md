# The desktop webview always receives the master token

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If something ever managed to inject script into the desktop window, it could capture the master web token and keep
minting new logins until the user rotates the token.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F33 / SEC-BOOTSTRAP-TOKEN-PUSHED`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/auth.rs:68-87`,
`crates/farhelm-ui/assets/desktop-auth.js:14`, `crates/farhelm-ui/assets/desktop-auth.js:77` — The desktop webview
receives the master web token on every (re)authentication, even when its stored secret is valid

Farhelm has two kinds of credential. The **web token** is the helm's root credential, printed by
`farhelm helm token
show`. It can mint new device logins without limit until it is rotated. A **device secret** is a
per-device credential minted from the web token, and it can be revoked or evicted from the helm's 64-entry table.

The desktop webview needs its own device secret. On every launch and every re-authentication, `DesktopBootstrapGate`
reads the web token from the helm's database and sends `{base, token, persisted}` into the page over the eval channel
(`auth.rs:68-87`). It does this before the page has checked whether its saved device secret still validates.
`desktop-auth.js` only uses the token when that validation returns 401 (the `exchange(bootstrap.token)` call around line
77). A protocol for requesting the token on demand already exists: the page's `retry_token` round trip, which fetches it
again after a 401 exchange. So the token is pushed into JavaScript even in the common case where it is not needed.

This is least-privilege hardening, not an exploitable bug on its own. The concern is what a future script injection
could steal. `desktop-auth.js` is re-evaluated on each re-authentication and takes `window.fetch` as it finds it at that
moment (`fetch: window.fetch.bind(window)`). A script already injected into the page could replace `fetch`, fake a 401
on the validation request, and receive the web token in the exchange request's body. That turns the compromise of one
revocable device secret into theft of the root credential. At launch this cannot happen, because authentication finishes
before `AppBody` renders any untrusted content. Only a re-authentication during an active injection is exposed. The
suggested fix is to leave the token out of the first message and send it only after validation returns 401, and
optionally to capture `fetch`, `WebSocket` and `localStorage` references when `desktop-auth.js` first loads.
