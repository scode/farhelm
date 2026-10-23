---
kind: added
---

The add host dialog takes an optional alias next to the destination, so a host can be added under the name you want to
see instead of renamed afterwards in its settings. The alias follows the same rules as in the host's settings: it must
not already be another host's name.

Adding a destination that is already one of your hosts is now refused, naming that host, before Farhelm connects to it.
Before, the add quietly set up the existing entry again. An existing host can still be set up again from its own row in
the hosts list.

A browser tab that was open before the update keeps the old behavior until it is reloaded.
