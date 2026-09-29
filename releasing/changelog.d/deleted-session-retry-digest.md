---
kind: fixed
---

Deleting a session no longer leaves its agent command line (and resume command) behind in the supervisor's database. A
record that stops the same create request from running twice is kept, as before, but now only as a digest.
