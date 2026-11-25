---
kind: fixed
---

If a session is deleted while its Restart is still starting the replacement agent, and Farhelm cannot clean that
replacement up, the supervisor now records the leftover cleanup and finishes it the next time it starts, even if no
sessions remain. Before it trusts that the agent's Linux scope is empty, it now waits for the failed launch to finish or
ends it.

Context for curation: the startup retry of a failed Restart's cleanup already shipped in v0.44.0. What is new is that a
replacement that failed before Restart confirmed it now gets that retry at all, that the retry checks the recorded
launch before an empty scope counts as clean, and that it leaves alone the processes of a session that now reuses the
deleted session's ID.
