# Security notes

NOTE: This is not a security review and not a threat model for the whole system. It documents what the code does at
specific trust boundaries, and the tradeoffs that were taken, so that a later review has something concrete to check
against. It covers only what it covers; a boundary not described here has not been written up yet, which says nothing
about whether it is sound. SPEC.md's Security section is the product contract; SPEC_impl.md's auth paragraph under Helm
internals is the design summary. This file is the longer form of the latter, verified against the code on 2026-08-30.

## The browser-to-helm credential

### What the code does

There are two secrets. The **web token** is the bootstrap secret: a random 128-bit value, base64url without padding (22
characters), minted on the helm's first need and stored in plaintext in `helm.db`'s single-row `web_token` table. It is
plaintext on purpose: `farhelm helm token show` has to print it, and it is the thing a user pastes into a browser once
per device. `token show` reads the database directly and works with or without a helm running.

A **device secret** is what a browser actually authenticates with. `POST /api/auth/token` takes the web token in a JSON
body, compares it (constant-time, over SHA-256 digests so the comparison is fixed-length) against the stored token, and
on a match mints a fresh 128-bit device secret, stores its SHA-256 in `device_sessions`, and returns the secret in the
response body. Only the digest is stored — the column has a `CHECK (length = 32)` so nothing but a digest fits — and the
table keeps the 64 newest rows, so a leaked web token cannot grow the database without bound. Token validation and the
row insert are one immediate SQLite transaction, which is what serializes an exchange against a rotation. An invalid
guess is rejected against an in-memory copy of the token before the database is touched, and a candidate that is not 22
base64url characters is rejected before hashing. There is no rate limit on this endpoint; the defense against guessing
is 128 bits of entropy.

The browser keeps the device secret in `localStorage` under `farhelm.device-secret` and presents it explicitly on every
protected edge: REST requests carry `Authorization: Bearer <secret>`, and WebSocket upgrades offer two subprotocols,
`farhelm` and `farhelm-device-<secret>`. The helm selects only `farhelm` in the upgrade response, so the credential is
never reflected back. Middleware (`require_device_session` in `crates/farhelm-helm/src/auth.rs`) sits on every `/api`
route except the exchange itself and the static bundle, as a router layer rather than a per-handler call, so a new route
cannot forget it. A WebSocket upgrade is authenticated by subprotocol only and a REST request by the header only;
neither accepts the other's transport. No cookie is set, read, or honored anywhere. (The `device_sessions.cookie_hash`
column name is a fossil from the design this replaced; it holds the device-secret digest.)

Rotation (`farhelm helm token rotate`) replaces the web token and deletes every `device_sessions` row in one
transaction, then broadcasts a process-local revocation that every live terminal and event-feed socket is selecting on;
those sockets drop and detach from the supervisor. Sockets subscribe to that broadcast before their handshake is
admitted, so a rotation cannot land in the gap between "authenticated" and "listening for revocation". When a helm is
serving, the CLI hands the rotation to it over a private unix socket in the 0700 state directory (peer uid checked) so
the running process is the one that both commits and revokes; when none is, the CLI rotates in the database directly,
and a lifetime `flock` keeps the two from racing. After rotation every existing device secret 401s and the browser shows
the token prompt again.

The desktop app is a variation on the same flow, not a different one. It embeds the helm, reads the web token straight
out of `helm.db` (same user, same machine), and holds two device sessions: one for its native reqwest client, kept in
process memory and never handed to JavaScript, and one exchanged inside the webview so that the webview's `localStorage`
and WebSocket subprotocol carry a credential of their own. The webview's secret is also persisted to
`desktop-client.json` (mode 0600) so a relaunch validates it via `GET /api/auth/device` instead of minting another. The
webview's origin is a custom scheme (`dioxus://`, `wry://`), so its fetches to the loopback helm are cross-origin; CORS
headers are attached to exactly the five routes it fetches (validate, exchange, upload, client-log, clipboard), echoing
only those custom-scheme origins.

Two things sit beside the credential and are worth knowing about because the tradeoff below leans on them. The loopback
origin guard (`require_loopback_origin`) refuses any request whose `Host` is not this helm's own loopback authority, any
browser `Origin` that is not that authority or a desktop custom scheme, and any top-level cross-site navigation
(`Sec-Fetch-Site: cross-site` with no vouching Origin). Every response carries `X-Frame-Options: DENY` and
`frame-ancestors 'none'`.

### The tradeoff that was taken

The device session was originally designed as an HttpOnly cookie. It was changed mid-build (PR #117) for a reason that
is easy to miss: cookies are scoped to a host, not a host and port. `localhost:7000` and `localhost:7001` share a cookie
jar. On a machine with more than one user, anyone who can bind a loopback port can serve a page at
`http://localhost:<their port>` and the browser will attach a farhelm cookie to requests there, which means the
credential leaves the helm's control without any bug in the helm. `localStorage` is scoped to the full origin, port
included, and an explicit header or subprotocol is only ever sent where the page's own code sends it. That is what the
switch bought, and it also removed the ambient credential that makes CSRF a category at all.

What it gave up is HttpOnly. Script running in the helm's origin — the app's own bundle, or anything that reaches script
execution there through an injection — can read the device secret out of `localStorage`. The judgment recorded in
SPEC_impl.md is that this adds little, because such a script can already call every API the secret authorizes from
inside the page; the credential is full authority and nothing is gated behind a second factor, so the injection has the
authority whether or not it can read the bytes. What HttpOnly would still have prevented is _exfiltration_: with
`localStorage`, an injection can send the secret out and the attacker then holds a standalone credential that works from
any client until the next rotation, rather than only for as long as their script runs in the user's tab. That is the
residual, and it is why every path from untrusted text into the DOM (session titles, cwds, host and provisioning output,
anything a terminal can turn into a link) has to be treated as a security boundary rather than a rendering concern.

Other properties of the current design, stated without judgment: device secrets never expire and are revoked only by
rotation, which revokes all of them at once (there is no per-device revocation); the exchange endpoint is public and
unthrottled; the web token is the same value forever until someone rotates it; and the device secret is transmitted in
the clear on every request, which is acceptable only because the edge is loopback-only and the spec forbids binding
anything else.

### The gap the browser path does not close, and the position taken on it

Everything above is about who may talk TO the helm. Nothing in it lets the browser confirm that the thing answering on
the helm's port IS the helm. Port-scoped storage stops a credential from reaching a different port; it does nothing
about a different process on the same port. Concretely: the helm is down (stopped, restarting, crashed), another user on
the same machine binds its port, and either a tab left open sends its stored device secret to them on the next
reconnect, or the user opens the bookmarked URL, sees a page that looks like Farhelm's token prompt, and pastes the web
token into it. The URL bar shows the same `http://localhost:<port>` either way, so there is nothing for the user to
notice. The preconditions are all required: another local user, the helm not running at that moment, and (for the paste
case) a convincing lookalike. On a machine with no other local users, none of this is reachable.

No client-side mechanism closes it. A key the page holds, a challenge the page answers, a token that carries the helm's
identity for the page to check — all of them are checked by code that, in this scenario, the attacker served. The
browser has exactly one way to verify a server before running its page, and that is a TLS certificate chained to a trust
anchor the browser already holds. For `localhost` no public CA will issue one, so it means the user installing a
Farhelm-minted CA into each browser's trust store on each client machine: a one-time, out-of-band configuration step per
machine, not something a link click can do. The other complete answer is to not use a browser at all.

The position, decided 2026-08-30 and deliberately: this is an accepted trade-off, not an oversight. The browser
interface is recommended only in single-user situations, meaning a helm machine (and any machine an SSH forward passes
through) where nobody else has a local account. The native application is the preferred client wherever it is available;
it embeds the helm and reads the token from disk, so there is no port and no page to impersonate. Farhelm does not ship
local TLS or a trust-store installer, and does not plan to for v1; SPEC.md's Security section states the assumption in
one sentence and points here.

For calibration, this is where nearly every local-web-UI tool sits. Jupyter runs token-authenticated plain HTTP on
loopback and tells users to add TLS themselves if the machine is shared; most local dev servers and tools with a
loopback API (Ollama, for one) have no authentication on loopback at all and leave the same-user assumption unstated;
code-server documents a password and recommends a TLS-terminating proxy in front. Syncthing is the one that ships a
self-signed certificate by default, and it does so because it targets NAS boxes and shared home servers. Products that
avoid the problem entirely (Docker Desktop, password managers) do it by being native applications with no browser path.
Stating the assumption out loud puts Farhelm ahead of most of that list; it does not make Farhelm unusual.

If the deployment story changes — shared machines become a target — the options, in rough order of cost, are: a
helm-minted CA plus a `cert install` command (the standard answer; friction is per-browser trust stores and managed
machines that forbid adding CAs); a browser extension using native messaging (no port at all, which is how password
managers talk to their local apps; heavy); or declaring the browser unsupported on such machines. Proof-of-possession
keys for the device credential (a DPoP-shaped design) are worth doing on their own merits — they stop an in-origin
script from exfiltrating a working credential — but they protect sessions after a good bootstrap and do not address the
impostor case, so they are a separate decision.
