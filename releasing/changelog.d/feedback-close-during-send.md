---
kind: fixed
---

Closing the **send feedback** dialog right after pressing **send**, with **cancel** or Escape before the dialog had
switched to "sending…", could close it while the message was still on its way, losing your text without saying whether
it arrived. The dialog now stays open until the send succeeds or fails, and a failed send keeps your text as it always
promised.
