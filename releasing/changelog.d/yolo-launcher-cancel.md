---
kind: fixed
---

Cancelling the "Confirm YOLO launch" question in the session launcher now always cancels it. Clicking **cancel** and
then, in quick succession, **start YOLO session anyway** or **start, and don't ask again on this host** could still
launch the session with no approval prompts (and, from **replace with**, delete the session it replaces), and the second
answer could also stop the host from asking before YOLO launches. While Farhelm records "don't ask again" for the host,
the question now goes away, and it comes back with the reason if that fails.
