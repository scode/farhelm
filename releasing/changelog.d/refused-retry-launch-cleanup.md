---
kind: fixed
pr: 918
---

A keyed session retry that is refused now removes the previous attempt's leftover launch files, so credentials from an abandoned launch do not remain on disk until the supervisor restarts.
