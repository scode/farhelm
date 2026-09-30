---
kind: fixed
---

Deleting a session whose fresh GitHub checkout was never fully set up (for example because the supervisor stopped while
cloning it) now tells you in the session list that the folder at its path was left untouched for you to inspect. Before,
this was only written to the supervisor's log.
