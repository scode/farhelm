---
kind: fixed
---

The install script no longer refuses forever with "another farhelm install/update is already running" when a lock left by a killed run happens to record the same process id the new run gets, which is common in containers.
