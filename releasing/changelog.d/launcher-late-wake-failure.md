---
kind: fixed
---

When a launch on a suspended sandbox or a stopped container fails to wake it, the session launcher now reopens at once
with the real reason, instead of waiting out a minute and then saying the host did not connect. When starting a stopped
container fails, the launcher now says that starting it failed, not resuming it.

Before, a wake that failed more than 20 seconds after the press, or that failed with the same error as an earlier
attempt, left the launcher waiting for its whole minute, and the real reason appeared only in the helm's log. Resume
from the Hosts panel or a session's controls still behaves as before.
