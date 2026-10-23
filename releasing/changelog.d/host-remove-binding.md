---
kind: fixed
---

Removing a host from the hosts panel now always removes the host its confirmation showed. Before, a Remove click still
queued when that confirmation was cancelled and another host's was opened could remove the other host instead. That
needed three inputs to land before the screen updated, and nobody has reproduced it.
