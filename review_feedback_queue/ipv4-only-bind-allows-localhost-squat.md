# IPv4-only bind lets another local user capture localhost while the helm runs

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a machine with another local account, opening the Farhelm UI at `http://localhost:<port>` can silently hand that
account full control of the user's agent terminals, even while the helm is running.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F15 / SEC-IPV6-SQUAT`, tagged **possible**. Anchors and title: `crates/farhelm-helm/src/lib.rs:1583-1587`,
`middleware.rs:116-117` — The helm binds only IPv4 loopback, so another local user can hold `[::1]` on the same port and
capture `http://localhost:<port>` while the helm runs

The helm listens only on IPv4 loopback: `SocketAddr::from((Ipv4Addr::LOCALHOST, args.port))` (`lib.rs:1583–1587`). It
never binds the IPv6 loopback address `[::1]`. On Linux, a different, unprivileged user can bind `[::1]:7433` while the
helm holds `127.0.0.1:7433`, because the two addresses do not conflict. On machines where `localhost` resolves to `::1`
first (true on the reviewed host; the reviewer checked with `getaddrinfo` and a throwaway socket test), a browser
opening `http://localhost:7433` may reach the squatter instead of the helm.

That matters because of how browser storage is scoped. The squatter's page is served at origin `http://localhost:7433`,
the same origin the real UI has when the user reaches it via `localhost`. Its script can therefore read the stored
device secret (`localStorage["farhelm.device-secret"]`) directly and send it to the squatter. The squatter's own process
then calls the real helm on `127.0.0.1:7433` with that secret. It is an ordinary local client, not a browser, so CORS
does not apply, and the origin guard accepts `localhost:7433` / `[::1]:7433` host values (`middleware.rs:116–117`). That
is full helm authority, including typing into agent terminals on every host. `docs/browser-limitations.md` presents
`http://localhost:<port>` as a normal way to open the UI.

SPEC "## Security" and `docs/security.md` accept port-squatting by another local user only "while the helm is down";
`docs/security.md` lists "the helm not running at that moment" as a required precondition. Binding only IPv4 removes
that precondition for anyone who uses the `localhost` name.

The unverified premise is browser behaviour: that Chrome, Firefox, and Safari try `::1` first for `localhost` and use it
when something answers there. `getaddrinfo` does so on the reviewed host, but this was not observed in a real browser.

Suggested fix: also bind `[::1]:<port>`, logging and continuing if IPv6 loopback is unavailable. Or stop accepting
`localhost` and `[::1]` in `origin_is_allowed` so the UI works only at `127.0.0.1`, and update
`docs/browser-limitations.md` to match.

Restater note: the same SPEC paragraph also says the browser UI "is recommended only on a machine with no other,
untrusted local users", and this attack needs such a user. That recommendation narrows who is exposed. The explicit
acceptance, however, is worded only for the helm-down case, so whether this is covered is the maintainer's call.
