---
kind: fixed
---

When a restart failed in a way that left it unclear whether the new agent had started, the session could keep pointing
at its old, closed terminal: opening it showed nothing, and the next restart could stop the new agent without asking.
The session now shows no terminal until Farhelm rediscovers it, and a restart asks first as it does for any session
whose agent may be running.
