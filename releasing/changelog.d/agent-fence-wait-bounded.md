---
kind: fixed
---

A `farhelm agent` command that changes something (stop, restart, rename, create, or clone) no longer waits up to ten minutes when an earlier such command from the same session is still unanswered. It now gives up after about half a minute with an error saying the earlier change is still in progress and that retrying later is safe. Before, the waiting command could still take effect minutes after the agent had given up on it.
