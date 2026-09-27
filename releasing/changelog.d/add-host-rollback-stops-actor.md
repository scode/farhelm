---
kind: fixed
---

When adding a host fails and the helm rolls the registration back, it now also stops any connection it had already started to that host. Before, in rare cases a host the helm said it had not registered could still appear in the host list, possibly connected, and could not be removed until the next host change. The failure message also no longer contains a long run of spaces.
