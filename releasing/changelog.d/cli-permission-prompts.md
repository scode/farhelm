---
kind: breaking
---

Farhelm now asks you before an agent's `farhelm` command changes anything. When an agent (or anything else running in
a session) runs `farhelm spawn` or `farhelm agent create`, `clone`, `rename`, `stop` or `restart`, a card appears in
the corner of the Farhelm window showing what it would do, with **allow**, **always allow from** the agent's host, and
**deny**. The agent waits up to nine minutes for your answer; with no Farhelm window open, the command is refused at
once. Listings never ask. To stop being asked for a host, turn on **run farhelm commands from this host without
asking** in that host's settings, or answer a card with **always allow**.

Agents can no longer start YOLO sessions, or sessions that run a command line, on a host that asks before YOLO
launches; turn on **start YOLO sessions here without asking** for a host to allow that. The `--confirm-yolo` flag is
gone, and `farhelm spawn --inherit-agent` now needs the session to be open in Farhelm, like every other spawn.
