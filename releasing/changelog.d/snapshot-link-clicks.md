---
kind: fixed
---

Web and file links work again in the frozen terminal screen kept after a refused reconnect or takeover.

Context for curation: a later unreleased change (#2597) drops a link press held while the terminal reconnects, so
releasing it after the old screen comes back no longer opens the link; before, a drag away from the link made during
the reconnect was missed.
