---
kind: fixed
---

Confirming setup, update or uninstall of a host now always runs the plan that confirmation showed. Before, a confirm
click still queued when a newer plan replaced the one on screen could run the newer plan, and a stale Cancel or Escape
could close the plan now on screen. That needed inputs to land before the screen updated, and nobody has reproduced it.
