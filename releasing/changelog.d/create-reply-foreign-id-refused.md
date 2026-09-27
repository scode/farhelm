---
kind: fixed
---

Creating a session on a host that answers with a session id another host already uses now fails with a conflict naming the host, instead of reporting success. Before, opening, typing into, or stopping that "new" session could act on the other machine's existing session for a few seconds. Only a buggy or compromised host answers this way; the session it made may still exist there.
