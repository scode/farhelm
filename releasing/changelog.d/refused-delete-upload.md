---
kind: fixed
---

If a session delete is refused during its safety checks, Farhelm now leaves in-flight attachment uploads available for a retry. A successful delete still cancels uploads before removing the session.
