---
kind: added
---

The Hosts button now marks available remote-host updates, with a session-list reminder that can update the hosts it
names. Settings also has **Update remote hosts automatically**, off by default, which works even with no window open.

Optional updates use a dismissible card; a banner explains required updates that prevent using the host's sessions.
Both can open Hosts to inspect them. Automatic updates use the same checks and progress as manual Update and never
update the helm's own machine. A failed or refused automatic attempt waits for a new host connection or a helm restart
before trying again; manual Update remains available.
