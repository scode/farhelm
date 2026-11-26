---
kind: fixed
---

Saving a new Tensorlake API key, or clearing it, in Settings → Tensorlake or Settings → Credentials now closes an open
Delete question under Sandboxes not on this helm and clears the old list at once. The list then shows only what a fresh
check with the new key finds.

Before, the question and the rows the old key had found could stay on screen after the key changed, most visibly after
changing the key in Credentials and coming back to Tensorlake, and confirming that Delete would have acted on a sandbox
listed under the previous key. The answer to a Delete still in progress when the key changes no longer shows its result
under the new key's list.
