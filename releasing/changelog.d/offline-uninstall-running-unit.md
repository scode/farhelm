---
kind: fixed
---

Retrying an uninstall while Farhelm can't connect to the host is now refused while the host's supervisor is still
running, because Farhelm can't check its sessions. Before, if the supervisor was still running but Farhelm could not
reach it (for example because a custom command name no longer worked), the retry stopped it and its sessions without
checking. Once the host connects, or the supervisor has stopped, uninstall can be chosen again.
