---
kind: fixed
---

Restarting a session could still occasionally close it, leaving the main pane empty or switching to another session,
when the session list happened to refresh during the restart. A session that is restarting now stays in the list,
showing the state it had before the restart until the new run reports its own.
