---
kind: fixed
---

Building a Tensorlake recipe or deleting it leaves unrelated sandboxes with the same build name and their snapshots
alone. If that name remains occupied, the build fails with advice to wait or remove the sandbox in Tensorlake.
