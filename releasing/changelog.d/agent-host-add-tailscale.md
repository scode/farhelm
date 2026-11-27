---
kind: added
---

Agents can now ask for a Tailscale host when they add one, with `farhelm agent host add --tailscale`. The approval card
and the Add host dialog show the request, and you can still turn the option off before adding the host.

The plan behind this change says in its own Release note section that no release note is needed, so the curator may
drop this entry.

The change also bumps the protocol version between the desktop app and its hosts.
