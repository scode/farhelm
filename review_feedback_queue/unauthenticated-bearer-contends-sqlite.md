# Unauthenticated bearer values are looked up in SQLite without a shape check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Another local account could make the helm UI slow or unresponsive by flooding it with fake credentials, without any
valid secret.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F17 / SEC-BEARER-SQLITE`, tagged **possible**. Anchors and title: `auth.rs:266-272`, `auth.rs:316-324`,
`auth.rs:166-171`, `store.rs:3081` — Any `Bearer` value from an unauthenticated caller is hashed and looked up in SQLite
with no shape check

Every protected route runs `require_device_session` (`auth.rs:266–272`). It takes whatever text follows `Bearer` in the
`Authorization` header (`authorization_device_secret`, `auth.rs:316–324`), or whatever follows the `farhelm-device-`
WebSocket subprotocol prefix, at any length and in any shape. It hashes that value and checks it against the database
(`accepts_device`, `auth.rs:166–171` → `has_device_session`, `store.rs:3081`). The lookup runs on the blocking thread
pool and takes the store's single `std::sync::Mutex<Connection>`, the same lock that serves session listing, host-cache
refreshes, token rotation, and every other database access. A request with no credential at all is rejected without
touching the database. Only a supplied value reaches it.

The public token-exchange route does the opposite on purpose. It checks `valid_secret_shape` (exactly 22 base64url
characters) first, and then compares against a token cached in memory. `AuthState`'s docs (`auth.rs:73–77`) say the
cache exists "so an invalid public exchange never contends for SQLite". The device-secret path has no such protection. A
caller with no valid credentials, for example another local user who can reach the loopback port with curl, can make
every request it sends take the helm's one database lock.

That asymmetry is certain. Whether a loopback flood of fake credentials hurts noticeably more than a flood of plain HTTP
requests was not measured.

Suggested fix: apply `valid_secret_shape` to the extracted secret before hashing or touching the database (device
secrets always come from `mint_secret` as 22 base64url characters), and answer `unauthenticated()` otherwise. Optionally
add a small in-memory negative check.

Restater note: the shape check turns away only malformed values. An attacker who sends random, correctly shaped
22-character strings still reaches SQLite on every request, and an in-memory negative cache would not help against
values that never repeat. Closing the gap the way the exchange path does would need an in-memory set of valid device
hashes (at most 64, per `MAX_DEVICE_SESSIONS`) kept in step with exchange, eviction, and rotation. As suggested, the fix
mostly makes the two paths consistent. It does not bound the flood.
