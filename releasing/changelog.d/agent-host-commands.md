---
kind: added
---

Agents can ask to add a host with `farhelm agent host add DEST [--alias NAME]`, change its alias, icon or color with
`farhelm agent host edit`, or remove it with `farhelm agent host remove`. Each request needs your approval every time: the host's "run farhelm
commands from this host without asking" setting never covers it, and there is no Always allow. An agent cannot remove
this machine or the host its own session runs on.

Edit cards show each changed field's old and new value. Remove cards explain that Farhelm forgets the host while its
supervisor and sessions keep running; adding the destination again finds them.

Add opens the ordinary dialog with editable destination and alias. Accepted means approved, not installed: adding can
still fail, need setup or be cancelled, so the agent checks the hosts listing before relying on the host. An agent add
always asks before setup even when you have chosen to skip that question for your own additions.
