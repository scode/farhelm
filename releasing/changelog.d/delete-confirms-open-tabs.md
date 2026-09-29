---
kind: fixed
---

Deleting a session whose agent has stopped or exited now asks first when the session still has terminal tabs open, and the confirmation says how many tabs will close. Before, one click on Delete silently killed whatever was still running in those tabs, such as a dev server or a build. The Replace confirmation also mentions open tabs now, since replacing deletes the old session.
