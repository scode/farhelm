---
kind: fixed
---

After the helm restarted, a new session started from a page left open since before the restart (a laptop woken with the
new-session dialog open, say) could, in a rare case, launch on a different machine than the one the page showed, if
that host had meanwhile been pointed at another install. Such a stale request is now refused, as it always should have
been.
