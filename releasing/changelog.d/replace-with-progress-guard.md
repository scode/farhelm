---
kind: none
---

Replace with now clears its "Replacing…" progress the way plain Replace already did, even when its request is abandoned
before the reply. No ordinary path abandons one: the launcher cannot be closed while its request is being sent, so
nothing changes for users.
