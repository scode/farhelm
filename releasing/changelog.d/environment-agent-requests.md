---
kind: added
---

Agents in sandbox and container sessions can now ask Farhelm to act within their own sandbox or container: start more
sessions there, and rename, stop, restart or delete the sessions in it. Agents on other hosts can now start a session in
a new sandbox or container from one of your recipes, after you allow it on a card that always asks.

Before this, every request from an agent in a sandbox or container was refused, and no agent could create one. Requests
from inside that reach further, such as other hosts, templates or host changes, are still refused, and their requests
ask you as on any other host. Deleting another session from inside a sandbox or container removes only that session;
your own Delete of any session in it still removes the whole sandbox or container. Remote hosts need updating, since
the change bumps the protocol.
