---
kind: fixed
---

A host's Farhelm now refuses to start, with a message naming the path, when the farhelm program or its state folder sits
at a path that is not valid UTF-8. Before, it started, said only that some conversation tracking would be missing, and
then failed every new session and restart with a confusing error.
