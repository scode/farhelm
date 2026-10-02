---
kind: fixed
---

Replacing a session from its row in the session list, when the replacement needs the YOLO confirmation, now keeps the
answer of the prompt you confirmed. If that prompt said nothing was running and the session was restarted while the
YOLO question was open, the old session is no longer deleted out from under the new run; both sessions are kept.
