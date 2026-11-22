---
kind: added
---

Add host and Host settings now offer a Tailscale host option. Farhelm can resolve the node name and verify its SSH host
key through Tailscale when the host advertises its key on your tailnet.

Caveats for curation: Tailscale must be running on the computer running Farhelm, and the node must advertise the SSH
host key its server presents (a node running only its own SSH server may not); otherwise Farhelm says why it could not
verify the host. Turning the option on or off for an existing host reconnects it, and is refused while the host is
being checked or set up, or while it still runs environments or holds environment images Farhelm built there.
