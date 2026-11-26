---
kind: fixed
---

Stop no longer reports success on a session whose start never finished, such as one whose creation was interrupted. With nothing yet running that Stop could stop, it now refuses and points you to Delete, which deals with whatever the interrupted start left behind, or to Restart where the session offers it. Before, Stop could say the session had stopped while its agent was still starting.
