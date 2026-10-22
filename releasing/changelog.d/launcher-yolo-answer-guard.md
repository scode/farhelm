---
kind: fixed
---

In the desktop app, Replace with in the session launcher no longer deletes the old session if it started running in
the moment between your answer or key press and the launch. The old session is kept, matching what the launcher showed
when you acted; the new session is still started, and the launcher reports that both now exist. The window was a
fraction of a second, and nobody has reproduced it.

This covered answering the YOLO question with "start YOLO session anyway" or "start, and don't ask again on this host",
and launching from a launcher choice or a recent row with Enter. The web app already launched from the screen the user
acted on.
