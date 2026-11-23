---
kind: added
---

Farhelm can launch registered custom agents with `farhelm agent create`, `farhelm spawn`, and launch templates.
Use `--conversation` to start a resumable agent on a conversation ID you already know. A custom agent that reports
its conversation ID and provides a resume command can also reopen its conversation on Restart.

Custom agents count as YOLO and supply their own reporting hook. The desktop registration and launcher controls
arrive in the next change for this plan.
