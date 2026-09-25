# Any Dioxus/wry webview passes the origin guard and CORS

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None unless another Dioxus/wry app displays hostile content; then that content gets past the origin check but still
cannot act without a Farhelm credential.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F11 / COR-DIOXUS-PREFIX`, tagged **possible**. Anchors and title: `middleware.rs:167-169` — Any Dioxus or wry desktop
app's webview passes the origin guard and gets CORS, not just Farhelm's

The desktop app loads the UI from a custom URL scheme inside a native webview, so its requests to the loopback helm
carry an `Origin` like `dioxus://index.html`. `is_desktop_webview_origin` (lines 167–169) recognizes the desktop app by
checking only that the `Origin` starts with `dioxus://` or `wry://`. Two places trust that check. `origin_is_allowed`
lets such an origin through on every route, including both WebSockets. `desktop_webview_cors` echoes it back in
`Access-Control-Allow-Origin` on the five desktop routes (credential validation, token exchange, uploads, client-log,
clipboard), which lets that page read the responses.

`dioxus://` is the default scheme of every Dioxus desktop app, and `wry://` is a common scheme for apps built on wry
(the webview library underneath Dioxus). Any other app on the machine built that way, if it displays attacker-influenced
content, sends an `Origin` the helm treats as its own desktop client. The code comments justify allowing custom schemes
because "a web page cannot forge a custom-scheme `Origin`". That holds for web pages, but a page inside another native
app is not forging anything.

Impact is limited. Such a page still needs a device secret or the web token before it can do anything. The guard is the
documented barrier against foreign pages, and this is a hole in it. The premise is that another Dioxus/wry app on the
machine shows hostile content.

Suggested fix: match only the exact origin or origins Farhelm's desktop build produces (for example
`dioxus://index.html` plus any confirmed per-platform variants). Keep that list as the single definition the guard and
CORS already share.
