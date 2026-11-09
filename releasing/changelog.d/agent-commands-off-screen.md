---
kind: fixed
---

Agents can now use `farhelm agent` commands and `farhelm spawn` from sessions that are not open on screen. Before, an
agent in any session other than the one you were looking at was refused with "no helm is attached to this session" even
though Farhelm was connected. Remote hosts need updating for this change.

When Farhelm really is closed or cannot reach the host, the refusal now says "no helm is connected to this host" and
suggests opening Farhelm and checking that the host is connected, instead of telling the user to open the session.
Approval cards are unchanged. The old behavior dates back to when agent commands were introduced, so this is not a
regression fix: it is the first release where background sessions (for example one that launches and cleans up other
sessions) can use agent commands at all.
