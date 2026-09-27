---
kind: fixed
---

If Farhelm cannot resolve a new session's working directory when creating it (for example because part of the path changed at that moment), the create now fails with an error you can retry. Before, the session was created anyway, but restarting it could later be refused with "working directory now resolves to …" for as long as the session existed.
