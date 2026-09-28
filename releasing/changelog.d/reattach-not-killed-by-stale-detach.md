---
kind: fixed
---

A terminal you close and reopen right away is no longer sometimes closed again at once with a "session terminal ended", "output stream failed", or stalled message. When the earlier view of that terminal had just ended or stalled, its late cleanup could mistake the reopened view for itself and shut it down even though it was working.
