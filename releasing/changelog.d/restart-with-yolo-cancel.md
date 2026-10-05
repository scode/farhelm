---
kind: fixed
---

Cancelling the "Confirm YOLO launch" question inside **restart with** now always cancels it. Clicking **cancel** and
then, in quick succession, **start YOLO session anyway** or **start, and don't ask again on this host** could still
restart the agent with no approval prompts, stopping it first if it was working; the second answer was then treated as a
one-off yes rather than refused. An answer now applies only to the settings the question asked about: if you change the
settings after the question appears, restart again to be asked about the new ones. While Farhelm records "don't ask
again" for the host, the question goes away, and it comes back with the reason if that fails.
