---
kind: fixed
---

Dropping or pasting a file into a terminal no longer fails silently when the terminal reconnects or the session restarts
during the upload. The terminal now says the upload was interrupted and names the file: if the file was still being
checked, nothing was uploaded; if it was already on its way, it may have reached the host anyway, but its path was not
inserted. A failure or a "path not inserted" message already on screen also stays there through a reconnect instead of
disappearing.
