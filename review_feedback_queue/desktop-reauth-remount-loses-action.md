# Desktop reauth remounts the app and loses the triggering action

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the desktop app, the first click after a token rotation (delete, stop, create) makes the whole window reload, and the
action may or may not have happened, with no message either way.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F4 / COR-DESKTOP-REAUTH-REMOUNT-LOSES-ACTION`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/api.rs:1268`, `crates/farhelm-ui/src/auth.rs:191`, `crates/farhelm-ui/src/auth.rs:197` — The
desktop credential refresh remounts the whole app before the retried request runs, so the triggering action is silently
lost

The desktop app keeps two credentials. The native side (reqwest) has its own device secret, and the webview has a
separate one for its WebSocket and localStorage. `DesktopBootstrapGate` (`auth.rs`) is the component that shows
"Starting Farhelm…" until the webview's credential is ready, and only then renders `AppBody`, the rest of the app.

When a native request is refused as unauthenticated and this caller is the one that actually minted the replacement
(`replaced == true`), `retry_desktop_request` calls `require_desktop_webview_reauth()` before it builds and sends the
retry (`api.rs:1268`). That function increments `DESKTOP_AUTH_GENERATION`. The gate reacts in an effect (`auth.rs:188`
to `:197`): it sets `ready = false`, restarts its authentication future, and stops rendering `AppBody`. Unmounting
`AppBody` unmounts `ListView` and `SessionView`, and in Dioxus a component-scoped `spawn` task is dropped when its
component unmounts. The task that is awaiting this very retry is one of those: `do_delete`, `on_stop`, restart, rename,
create, or a listing read. So the retry is cancelled at its next await point. It may never reach the helm, or it may
reach the helm and have its reply discarded. Either way the UI shows neither success nor error. When the gate becomes
ready again it mounts a fresh `AppBody`, so open forms, pending confirmations and rename drafts are gone as well.

This is independent of F3. Even with the double-header bug fixed, the mutation that triggered the refresh vanishes with
an unknown outcome. F3's visible "authentication is required" error only reaches the user for requests that survive the
remount, such as the `spawn_forever` writers for preferences and seen state.

The recovery therefore destroys the very caller it is retrying for. After a rotation, a Delete or Stop click has an
unknown outcome and gives no feedback. The suggested fix is to delay the webview re-authentication until the retried
response is in hand, for example by returning `replaced` to `send_inner` and triggering the reauth afterwards. The
alternative is to re-authenticate the webview without unmounting `AppBody`.
