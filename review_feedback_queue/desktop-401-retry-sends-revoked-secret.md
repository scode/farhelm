# The desktop 401 retry re-sends the revoked secret first

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the desktop app, the first action after the helm token is rotated fails with "authentication is required" (or is
lost), even though the app already fetched a fresh credential, and the user has to repeat it.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F3 / COR-DESKTOP-401-RETRY-DOUBLE-HEADER`, tagged **definite**. Anchors and title: `crates/farhelm-ui/src/api.rs:1189`,
`crates/farhelm-ui/src/api.rs:1192`, `crates/farhelm-ui/src/api.rs:1272`, `crates/farhelm-helm/src/auth.rs:317` — The
desktop 401 recovery retries with the revoked credential still first, so the retry always fails

In the desktop app, native Rust code (reqwest) makes every REST call to the embedded helm and authenticates with a
**device secret**, a per-device credential sent as `Authorization: Bearer <secret>`. That secret can become invalid:
when the user rotates the helm's token, or when the helm's 64-entry device table evicts it. For that case, `send`
documents a "refresh and retry once" recovery.

The recovery is built like this. `send_inner` attaches the current secret with `request.bearer_auth(old)` and then
clones the builder with `try_clone()` to keep a copy for the retry (lines 1189 and 1192). So the clone already contains
the old `Authorization` header. When the helm answers 401 with its `device_auth_required` marker,
`retry_desktop_request` gets a fresh secret from `refresh_native_device` and calls `retry.bearer_auth(new)` (line 1272).
In the locked reqwest 0.12.28, `bearer_auth` goes through `header_sensitive`, which uses `HeaderMap::append`
(`async_impl/request.rs`, confirmed in the registry source). The retry therefore goes out with two `Authorization`
headers, the revoked one first. On the helm side, `authorization_device_secret` (`farhelm-helm/src/auth.rs:317`) reads
`headers.get(AUTHORIZATION)`, and that returns the first value. The helm rejects the revoked secret again, and the
caller receives `SendError::Unauthenticated`, shown as "authentication is required".

A second concurrent caller that finds the refresh already done (`replaced == false`) reuses the same clone and fails the
same way. Neither existing check notices. The unit test for this path only checks the deadline arithmetic, and the
desktop smoke's rotation leg only checks that the secrets on disk changed.

As a result, the documented recovery never succeeds after a rotation or an eviction, and the first action after one
fails even though a fresh credential was already obtained. The suggested fix is to attach the credential per attempt,
cloning the builder before `bearer_auth`, or to `insert` rather than `append` the header on the retry. A test should
assert that the retried request carries exactly one `Authorization` value, holding the new secret.
