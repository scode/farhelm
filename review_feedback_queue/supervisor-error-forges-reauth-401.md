# A remote supervisor can forge the helm's log-in-again 401

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A compromised remote host can repeatedly kick the browser UI to "paste your token" whenever the user touches one of its
sessions; repeated re-entry can eventually log out other browsers.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F16 / SEC-FORGED-REAUTH`, tagged **definite**. Anchors and title: `crates/farhelm-helm/src/lib.rs:1822-1835`,
`auth.rs:40`, `auth.rs:383`, `client.rs:2362`, `crates/farhelm-ui/src/api.rs:1310-1316` — A remote supervisor can forge
the helm's "log in again" 401 and throw the web UI to the token prompt

When a browser's device secret is missing or invalid, the helm's authentication middleware answers 401 with the body
`{"error":"unauthenticated","code":"device_auth_required"}` (`auth.rs:383`, marker constant at `auth.rs:40`). The web UI
treats that marker as "this browser is no longer logged in". Its `device_auth_required` check
(`crates/farhelm-ui/src/api.rs:1310–1316`) parses any 401 body as JSON and looks for that `code`. When it matches, the
browser build replaces the whole app with the paste-your-token prompt. The check's own doc comment assumes "Only the
authentication middleware emits this structured error code".

A remote supervisor can emit it too. Supervisors reply to requests with `ControlMsg::Error { kind, message }`, and the
helm's client turns that unchanged into a `SupervisorError` whose display text is just `message` (`client.rs:2362`).
`http_error` (`lib.rs:1822–1835`) maps `ErrorKind::Unauthorized` to HTTP 401 and uses the error text as the response
body. On routes that add no extra context, such as `POST /api/sessions/{id}/stop` (`do_stop_session` passes the client's
error straight through), a supervisor that answers with `kind: Unauthorized` and the marker JSON as its message produces
a 401 byte-for-byte identical to the helm's own.

SPEC "Local authority and trust between hosts" says the helm and GUI "must treat remote supervisor messages … as
untrusted", and running agents without permission checks on disposable remote hosts is an intended use, so an agent
there may well control or replace its supervisor. SPEC "Remote input, session defaults, and availability" adds that
malicious behaviour from a remote host "must not disrupt … ordinary helm/GUI controls". Here, a compromised host logs
the whole browser UI out every time the user acts on one of its sessions. Each re-paste of the token mints a new device
secret, and the helm keeps only the 64 newest (`MAX_DEVICE_SESSIONS`). Enough forced re-logins can therefore also evict
the credentials of the user's other browsers. The desktop build limits the damage: on this marker it re-validates its
native credential, retries once, and fails only that request rather than showing the prompt.

Suggested fix: make the signal impossible for a supervisor to forge. One way is to never answer 401 for errors that
originate from a supervisor; map `ErrorKind::Unauthorized` to 403, which matches what that kind means for a supervisor.
Another is to carry the device-auth signal in something only the middleware sets, such as a dedicated response header,
and have the UI key on that.
