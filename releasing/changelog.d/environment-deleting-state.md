---
kind: changed
---

While Farhelm deletes a sandbox or container, Hosts and the sidebar now show it as **Deleting…** instead of as a host
that cannot be reached. If deleting it fails after its last session was deleted, it shows **Deleting failed** with the
reason, and Retry deletes it again.

This is the part of the Delete change that is visible on its own; the rest (Delete keeping a sandbox or container while
other sessions remain) is described in environment-agent-requests.md and lands with the next change. The curator may
fold the two together.
