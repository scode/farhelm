---
kind: added
---

Agents in sandbox and container sessions can now ask Farhelm to act within their own sandbox or container: start more
sessions there, and rename, stop, restart or delete the sessions in it. Their requests ask you as on any other host.
Requests that reach outside, such as other hosts, templates or host changes, are still refused.

Before this, every request from such an agent was refused. Deleting another session from inside the sandbox or
container removes only that session; your own Delete of any session in it still removes the whole sandbox or container.
