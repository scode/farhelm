---
kind: fixed
---

A browser tab or app window that disappeared without closing its connection, for example on a laptop that went to sleep
or lost its network, now gives up its place in the helm's live-update feed within about a minute, even while sessions
are changing often. Before, frequent changes kept postponing that check, and the place was held until the network
connection itself timed out, which can take many minutes.
