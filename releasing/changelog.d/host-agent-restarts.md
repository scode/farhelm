---
kind: none
---

Nothing user-facing yet: restarting a session whose agent runs in a terminal host now starts the agent in a new host
that keeps the old terminal's scrollback (not its last screen) and ends the old host. A supervisor that crashed in the
middle of such a restart sorts it out when it next starts: it keeps the old terminal when the new one never started, and
ends the old host when the new one did. Only sessions created under a hidden harness flag that no shipped launch passes
run in terminal hosts, so every session still runs in tmux until a later change turns new terminals on; that change
carries the real entry.
