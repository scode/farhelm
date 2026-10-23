---
kind: fixed
---

Quitting the desktop app and reopening it while its supervisor was still finishing startup cleanup could make the new
supervisor's first launch fail: the old supervisor's cleanup could delete the checkout or launch files the new one had
just prepared. A stopping supervisor now keeps its hold on its state folder until all of its background work has
ended, so the new one waits for it instead. The same applied to a supervisor started by hand and restarted quickly.

If that cleanup takes longer than about 20 seconds, the reopened app now reports that the previous supervisor is still
shutting down instead of starting, and opening it again once the cleanup has finished works.

For curation: the deleted checkout only concerns prepared checkouts, which have not shipped yet (`spare-checkout.md`
is still a fragment), so for users of the last release the problem was the launch files alone.
