# Browser limitations: why the UI wants a localhost tunnel

What you lose when the web UI is reached at anything other than `http://localhost:<port>` (or `127.0.0.1` / `[::1]`),
and why. Reaching a remote helm means an SSH port forward you set up yourself (`ssh -N -L 7433:127.0.0.1:7433 box`), and
the helm refuses non-loopback binds anyway, so in v1 the non-tunnel case is mostly something you get into by accident: a
reverse proxy, a tailnet hostname, a forward on a different local port. This page says what breaks so the report is
"origin problem", not "clipboard is broken".

NOTE: this is not a description of the security model. For that, read SPEC.md's Security section.

## TLDR

Off a loopback origin over plain `http://`:

- The helm 403s every request whose `Host` header is not loopback on its own bind port. Nothing works at all. A forward
  to a different local port (`-L 8000:127.0.0.1:7433`) hits this too, so forward to the same port.
- If you get past that (a future TLS-less non-loopback bind, say), the programmatic clipboard is gone: copy-on-select
  puts nothing on the clipboard, and OSC 52 writes from programs inside the session (tmux, editors, agent CLIs) go
  nowhere. Both fail silently by design, so nothing tells you.
- Keystroke paste (Cmd+V, including pasted screenshots) still works.
- "Install as app" / "Add to Dock" is not offered, so no standalone window.

Over `https://` with a certificate the browser trusts, all of this comes back; that is the post-v1 TLS path.

## Why: secure contexts

Browsers gate a set of APIs behind a "secure context": `https://` origins, plus loopback over plain `http://`, which
counts because no network attacker can sit on it. That loopback exception is the whole reason the helm binds only
loopback and asks you to tunnel: it gets the gated APIs for free with no certificates and no user-installed CA.

The gate matters for the terminal because there are two clipboard mechanisms and only one is gated. The `paste` and
`copy` DOM events fire on a keystroke and work on any origin; the keystroke is the authorization. That is the path a
pasted screenshot takes (terminal.js reads `ev.clipboardData`, uploads the file to the helm, inserts the host-side
path). The Async Clipboard API (`navigator.clipboard.writeText()` / `readText()`) is what everything without a keystroke
needs, and it is marked `[SecureContext]`: on an insecure origin `navigator.clipboard` is `undefined`, not
present-but-denied. Copy-on-select and OSC 52 both go through it, which is why terminal.js writes
`navigator.clipboard?.writeText?.(text)?.catch(() => {})`: the optional chaining is the insecure-origin case and the
swallowed rejection is the engine-refused case, and both collapse into the silent-failure contract SPEC.md gives the
clipboard.

Secure context only makes the API exist. Each engine still applies its own permission policy on top (Chrome prompts for
reads, Safari wants a recent user gesture and treats a mouseup as a weaker gesture than a keystroke, Firefox restricts
reads further), so a loopback origin is necessary for copy-on-select, not sufficient. That second layer is what SPEC.md
means by "eligibility is not the same as success".

Installed web apps (Chrome's "Install app", Safari's "Add to Dock") sit behind the same gate, and the installed app is
bound to the exact origin, scheme, host and port included. A same-port forward to `localhost` satisfies both, which is
why the standalone-window experience costs nothing beyond a manifest; a non-loopback `http://` link satisfies neither.
