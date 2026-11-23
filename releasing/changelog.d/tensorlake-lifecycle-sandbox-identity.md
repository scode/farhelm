---
kind: fixed
---

Suspend and Resume now check that Farhelm can identify the Tensorlake sandbox for the host. A failed setup that only
reserved a name cannot suspend or resume another sandbox with that name.
