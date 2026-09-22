---
kind: fixed
pr: 843
---

Detaching from a terminal now always reaches the supervisor, even when the browser tab that asked for it went away before the detach finished. Previously such a session could stay marked as attached until the connection dropped.

Written from the diff at curation-seeding time; the PR has no description. Verify the "previously" sentence before it ships.
