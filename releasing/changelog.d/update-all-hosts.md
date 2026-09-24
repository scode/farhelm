---
kind: added
---

The Hosts header now has an `update all` action for remote hosts whose individual Update action is available. Each host shows its own progress and result; a failure on one does not stop the others. Hosts already busy with setup or another run are skipped, and the local host is never included.
