---
kind: fixed
---

A remote host whose registry entry is deleted while it is connected, through a path that does not stop its connection, now disconnects and leaves the host list at its next refresh. Before, it could stay listed as connected, with its connection open, until some later host change.
