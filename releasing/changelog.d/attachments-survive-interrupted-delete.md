---
kind: fixed
---

If Farhelm stopped in the middle of deleting a session, or a failed delete could not put the session's attached files
back, the session survived but those files were thrown away the next time Farhelm started. They are now put back with
the session.
