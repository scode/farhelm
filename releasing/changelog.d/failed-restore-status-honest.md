---
kind: fixed
---

In the rare case where a restart fails and Farhelm then also cannot write the session's previous status back to its database, the session now shows as unknown, matching what is stored, instead of showing its old status and later changing to unknown on its own after the host's supervisor restarts.
