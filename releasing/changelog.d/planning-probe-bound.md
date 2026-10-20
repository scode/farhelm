---
kind: fixed
---

Host planning no longer waits indefinitely for leftover diagnostic output, which could prevent new host setup, updates
and removals from starting.

This closes a possibility found by code reading; no SSH configuration that triggers it in practice has been
demonstrated. The separate install/update execution issue recorded in BUGS.md is unchanged.
