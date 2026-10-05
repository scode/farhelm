---
kind: fixed
---

Cancelling the "Confirm YOLO launch" question that can appear when you replace a session from the session list now
always cancels it. Clicking **cancel** and then, in quick succession, **start YOLO session anyway** or **start, and
don't ask again on this host** could still go ahead: the new session started with no approval prompts, the old one was
deleted, and the second answer could also stop the host from asking before YOLO launches. While Farhelm records "don't
ask again" for the host, the question now goes away, and it comes back with the reason if that fails.
