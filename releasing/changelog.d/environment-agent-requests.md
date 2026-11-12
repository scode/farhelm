---
kind: added
---

Agents in sandbox and container sessions can now ask Farhelm to act within their own sandbox or container: start more
sessions there, and rename, stop, restart or delete the sessions in it. Agents on other hosts can now start a session in
a new sandbox or container from one of your recipes, after you allow it on a card that always asks. Your own Delete of
a session in a sandbox or container now removes just that session while other sessions remain, and removes the sandbox
or container only with its last session.

Before this, every request from an agent in a sandbox or container was refused, no agent could create one, and your
Delete of any session in it removed the whole sandbox or container. Requests from inside that reach further, such as
other hosts, templates or host changes, are still refused, and their requests ask you as on any other host. Deleting
another session from inside a sandbox or container removes only that session, never the sandbox or container itself.

Delete always asks, and its confirmation states both outcomes. Deleting a session that is not the last tells you how
many other sessions are still in the sandbox or container ("The container stays: 2 other sessions are in it"), and its
checkout is deleted rather than moved to the checkout trash. When Farhelm cannot reach the Farhelm inside, because the
sandbox is suspended, the container is paused, either is unreachable, or its Farhelm needs an update, Delete offers to
delete the whole sandbox or container instead, with a warning that every session in it is destroyed, including ones
Farhelm does not list; resuming or updating it first lets Farhelm delete just the one session. Deleting several marked
sessions works through them one at a time the same way. **Delete sandbox** and **Delete container** in Hosts still
always delete the whole thing.

Remote hosts need updating, since the change bumps the protocol.
