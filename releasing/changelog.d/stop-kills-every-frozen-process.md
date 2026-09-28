---
kind: fixed
---

Stopping or deleting a session no longer sometimes leaves one of its processes frozen instead of ended, still holding things like a dev server's port. Stop pauses a session's processes before killing them; a paused process that the final kill pass could no longer see, for example one that had just started a setuid program such as `sudo`, was neither killed nor resumed.
