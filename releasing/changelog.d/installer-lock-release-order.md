---
kind: fixed
---

Running the installer twice in quick succession can no longer delete or silently revert the newly installed `farhelm`. After finishing its own install, the first installer still considered itself the owner of the install lock, so if a second installer took over the lock in the meantime, the first one's exit could undo the second one's changes while the second still reported success.
