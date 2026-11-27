---
kind: fixed
---

When an agent deletes a session and the Delete leaves something to clean up, the notices bell now always says so: a checkout left on disk, with the host, the path and the reason, or a Podman container the session started whose stop Farhelm could not confirm. Before, only the asking agent was told, and if its request ran past the reply deadline nobody was.
