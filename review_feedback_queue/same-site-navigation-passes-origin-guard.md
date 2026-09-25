# Pages on another loopback port pass the helm's navigation guard

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A page on another localhost port can make a tab open Farhelm and take over the terminal the user is working in ("another
client attached").

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F1 / COR-SAMESITE-NAV`, tagged **possible**. Anchors and title: `middleware.rs:136-151` — The top-level navigation
guard rejects only `cross-site`, so pages on another loopback port can still navigate a tab to the helm

Every request to the helm first passes an origin guard (`origin_is_allowed`). Its purpose is to stop a hostile web page
in the user's own browser from driving the helm. Most browser requests carry an `Origin` header, and the guard requires
it to name the helm's own loopback address. A top-level navigation, where a page sets `window.location` to the helm's
URL or calls `window.open` on it, sends no `Origin`. For that case the guard relies on `Sec-Fetch-Site`, a header the
browser sets and page script cannot forge, which says how the requesting page relates to the target. The guard rejects
only the value `cross-site`.

The comment above the check (lines 136–149) explains what it defends against. When the UI loads, it reselects a session
and attaches its terminal with the ordinary, displacing attach, so loading the page takes that terminal away from
whichever client held it. A hostile page that can make a tab load the helm can therefore kick the user's working window
off its terminal just by causing the load.

The gap is that browsers define a "site" as scheme plus host, ignoring the port. A navigation from
`http://127.0.0.1:3000` to `http://127.0.0.1:7433` is therefore `same-site`, not `cross-site`, and it passes. The loaded
page runs at the helm's real origin, finds the device secret in `localStorage`, and attaches as usual. The attacking
page must use the same host spelling the user normally uses (`127.0.0.1` vs `localhost`). A page on `localhost:3000`
navigating to `127.0.0.1:7433` is cross-site and is refused, and navigating to a spelling the user never used lands on a
token prompt with no secret. Browsers that send no `Sec-Fetch-Site` at all, such as Safari before 16.4, pass every
navigation.

The attacker cannot read the page, frame it (every response carries `X-Frame-Options: DENY`), or call the API, because
it never holds the secret. What it can do is repeatedly steal the terminal from the user's active client, which then
shows "another client attached". Any page served on another loopback port can do this: a dev server rendering untrusted
HTML, a notebook server, or another local user's server. `docs/security.md` promises only to refuse "top-level
cross-site navigation", so the code matches the doc. The miss is against the goal the comment states.

Suggested fix: when no allowed `Origin` vouches for the request, accept `Sec-Fetch-Site` only when it is `none` (address
bar or bookmark), `same-origin` (reload, or the app's own requests), or absent, and reject `same-site` along with
`cross-site`. Add `same-site` cases to the `origin_is_allowed` unit-test matrix and update the comment and
`docs/security.md`. The absent-header case needs its own decision. Allowing it keeps older Safari open, but refusing it
would also refuse curl and other non-browser clients, which send neither header.
