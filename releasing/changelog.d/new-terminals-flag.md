---
kind: none
---

No entry of its own: the supervisor gains a hidden `--new-terminals` option that chooses which kind of terminal new
sessions and tabs get. No shipped launch passes it; its default, terminal hosts, is what `terminal-hosts-on.md`'s entry
describes, and test harnesses pass `tmux` where they still test old terminals.

For the curator: this raises the supervisor database schema again (32 to 33) without changing anything stored, so that
the builds at 32, which record each session's terminal kind but cannot all handle a host terminal, refuse a database
such a supervisor has opened. The release's downgrade check reports it. Per the editorial guidance no entry says the
release cannot be downgraded from.
