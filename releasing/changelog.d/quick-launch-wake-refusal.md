---
kind: fixed
---

A quick launch that had to wake a suspended Tensorlake sandbox, and was then refused because something it needed was no
longer ready (the host stopped being available, say), now shows the session launcher with the reason. Before, the
launcher stayed invisible but still caught every click and key, so the window seemed frozen until you pressed Escape.

This needed a quick launch, or a template with a number key, whose destination is a new GitHub checkout on a suspended
sandbox, and a refusal that came without an error message. Refusals with an error message already showed the launcher.
