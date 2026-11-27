---
kind: fixed
---

In Settings → Credentials, going Back or pressing Cancel while a replaced credential was still saving no longer loses
the list of hosts it could not be delivered to. Back and Cancel are now unavailable until the helm answers the save or
Delete, which on an ordinary connection takes a moment, and the hosts then show on the Credentials overview.

When you close Settings and choose Save, a refused credential save now shows the credential's editor with the helm's
reason, even when an earlier refused Tensorlake API key save had brought the Credentials overview forward.
