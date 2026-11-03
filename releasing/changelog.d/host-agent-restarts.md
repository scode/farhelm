---
kind: none
---

No entry of its own: restarting a session whose agent runs in a terminal host starts the agent in a new host that keeps
the old terminal's scrollback (not its last screen) and ends the old host. A supervisor that crashed in the middle of
such a restart sorts it out when it next starts: it keeps the old terminal when the new one never started, and ends the
old host when the new one did. Every new session runs in a terminal host since `terminal-hosts-on.md`'s change, whose
entry covers this.
