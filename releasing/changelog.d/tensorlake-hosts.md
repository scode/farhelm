---
kind: added
---

You can now add a Tensorlake sandbox as a host from a saved recipe, with setup progress in Hosts. Creating it confirms
that it bills while running; deleting it also deletes its snapshots.

New Tensorlake hosts start YOLO sessions without asking. Reused images need systemd running as their init system and
must not contain an existing Farhelm supervisor installation.
