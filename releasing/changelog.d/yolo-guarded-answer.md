---
kind: added
---

In the session launcher and Restart with, the YOLO confirmation can now start the same launch in guarded mode instead.
The option appears when you explicitly chose YOLO and the agent offers guarded mode.

A follow-up fix makes the launcher's "start guarded session instead" honor what Replace with showed: when it waits for
a GitHub checkout preview, the old session is now kept if it was started again in the meantime, instead of being deleted
regardless, and the answer does nothing if the launcher was reopened for a different session first. No release shipped
the gap, so this needs no entry of its own.
