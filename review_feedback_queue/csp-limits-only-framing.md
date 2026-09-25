# The helm CSP limits only framing

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If a page-injection bug is ever found, nothing in the browser stops the stolen sign-in credential from being sent to an
outside server.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F36 / SEC-CSP-FRAMING-ONLY`,
tagged **possible**. Anchors and title: `crates/farhelm-helm/src/middleware.rs:82` — The helm's content security policy
only blocks framing, not where page script may send data

The helm's middleware sends exactly one Content-Security-Policy directive on every response: `frame-ancestors 'none'`
(line 82), alongside `X-Frame-Options: DENY`. Both exist to stop clickjacking. Nothing restricts where page script may
connect (`connect-src`), load images from (`img-src`), embed objects from (`object-src`), or where relative URLs resolve
(`base-uri`). In the browser build, the device secret lives in `localStorage`, readable by any script on the page.
`docs/security.md` names exfiltration of that secret as the main residual risk: today the only defence is the "no HTML
sinks" discipline, meaning no code path inserts untrusted text as markup. If an injection bug is ever found, nothing at
the browser level stops `fetch` or an image request from sending the secret to an outside host.

This is defence-in-depth hardening, not a present vulnerability, and even the stricter policy would be partial. CSP does
not block every exfiltration channel (top-level navigation, for instance), and Dioxus's web `eval` bridge plus the wasm
bundle need `'unsafe-eval'` / `'wasm-unsafe-eval'` in `script-src`, which weakens script restrictions. The suggested
policy is
`default-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none';
frame-ancestors 'none'`
plus the minimal `script-src` the wasm bundle needs, verified in both Chromium and WebKit.

Restater note: this policy only protects the browser build. The desktop webview loads its page from dioxus's `dioxus://`
custom protocol, not from the helm, so the helm's response headers never govern the desktop document, and no CSP is set
on that path (no `Content-Security-Policy` in `crates/farhelm-ui/src`). Hardening the desktop build would need a
separate change.
