---
kind: none
---

Nothing user-facing yet: the supervisor gains a hidden `--new-terminals` option that test harnesses use to start it with
terminal hosts turned on. No shipped launch passes it, so every session still runs in tmux until a later change turns
new terminals on; that change carries the real entry.

For the curator: this raises the supervisor database schema again (32 to 33) without changing anything stored, so that
the builds at 32, which record each session's terminal kind but cannot all handle a host terminal, refuse a database
such a supervisor has opened. The release's downgrade check reports it. Per the editorial guidance no entry says the
release cannot be downgraded from.
