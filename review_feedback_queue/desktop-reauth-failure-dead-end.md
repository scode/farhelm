# A transient desktop reauth failure leaves an unrecoverable error

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If re-authenticating the desktop window is briefly slow after a token rotation, the app is replaced by a single error
line and stays that way until it is quit and relaunched.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F5 / COR-DESKTOP-REAUTH-FAILURE-STUCK`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/auth.rs:54`,
`crates/farhelm-ui/src/auth.rs:127`, `crates/farhelm-ui/src/auth.rs:160`, `crates/farhelm-ui/src/auth.rs:188`,
`crates/farhelm-ui/src/auth.rs:199` — A transient failure during desktop webview re-authentication leaves the window on
an error with no way out

After a native 401 (see F4), `DesktopBootstrapGate` hides the app and runs the webview sign-in again, using
`assets/desktop-auth.js` over the Dioxus eval channel. That sign-in can fail in several ways:

- the token exchange hits its 5-second abort;
- the WebSocket greeting check in `accepted()` gives up after 5 seconds ("webview event socket failed after device
  exchange");
- validation returns a status other than 401;
- the IPC channel itself errors.

Every one of these ends with `failure.set(...)`, and the gate then renders a single `auth-error` paragraph where
`AppBody` would be.

Nothing ever leaves that state. The only thing that restarts the gate's future is another bump of
`DESKTOP_AUTH_GENERATION`, and the only caller of `require_desktop_webview_reauth()` is the native 401 retry path in
`api.rs`. That 401 cannot recur: native REST already holds a valid new credential, and with `AppBody` unmounted almost
nothing issues requests anyway. There is no retry button, timer or other trigger. A slow moment during an ordinary token
rotation therefore turns into a dead window, and only quitting and relaunching the app recovers it. The two 5-second
budgets are tight on a loaded machine, which is why this is plausible rather than theoretical. It is tagged "possible"
because it needs such a transient failure to line up with a rotation.

The suggested fix is to add a Retry control that calls `authentication.restart()`, and/or retry automatically with
backoff after transient failures, keeping a terminal error only for causes that cannot be retried.
