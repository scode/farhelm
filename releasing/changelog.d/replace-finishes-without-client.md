---
kind: fixed
---

Replacing a session no longer leaves the original behind when the window that asked for it goes away partway, for
example because it was reloaded or closed while a slow replacement was still being created. The new session used to be
created but the original never deleted, usually with no error shown. Farhelm now finishes the replace once it has
started, and does the same for creating, restarting, renaming and deleting a session and for marking it read or unread:
a window that goes away only misses the answer.
