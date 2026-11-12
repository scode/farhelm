---
kind: breaking
---

After you update the app, every remote host shows "needs update" until you update it too, because the app and its hosts
now speak a new version of their protocol. A host that has not been updated stays disconnected, and its sessions out of
reach, until then. Reload any browser tab that stayed open across the update: within a few seconds its terminals stop
reconnecting on their own, and any terminal it opens or reconnects before then, or that you reconnect, shows garbled
output.

Context for curation: this release drops the old terminal component (xterm.js) from the app, and with it the protocol's
description of old and new terminals; that change is what needs the protocol bump. Users see no other difference in
their terminals: every terminal was already drawn by the new one, and the session view no longer has a band about moving
to new terminals or names the terminal kinds in its bottom bar, since only one kind is left. This is a separate entry
from the one about old terminals being ended because it asks for a different action (updating every remote host).
