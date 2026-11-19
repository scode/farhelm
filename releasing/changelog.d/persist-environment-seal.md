---
kind: fixed
---

Farhelm retries deleting a sandbox or container after a helm restart if the last session's deletion had already
started removing it, even if the sandbox is now suspended. If deletion fails again, Hosts shows the reason and Retry.
