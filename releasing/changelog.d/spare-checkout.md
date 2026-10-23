---
kind: added
---

Managed-checkout launches can now reuse a checkout prepared earlier, with a switch under Settings → checkouts that's
on by default and costs one extra checkout's disk space per repository and host. Unchanged files keep older modification
times, and Git hooks and checkout filters run ahead of time and again only for the update.

Background preparation does not prompt for credentials; if it fails, a later launch can clone normally. If the branch
was rewound or rewritten upstream since the checkout was prepared, the launch clones normally instead of starting on
the old history. The [prepared-checkout guide](https://farhelm.io/docs/using/start-a-session/#prepared-checkouts)
describes how the earlier clone differs and when Farhelm discards it.
