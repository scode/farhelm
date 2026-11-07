---
kind: none
---

Nothing a user sees changes: the supervisor's database stops storing a tmux session name for every session and stops
keeping a new terminal's location in a field named for tmux panes. Old sessions keep running and are found as before.

Context for curation: this is a supervisor database upgrade (schema 34), one-way like the others, which by the editorial
guidance goes unsaid. The one case a user could meet is a database someone edited by hand so that a session's stored
tmux name no longer matches the session: the upgrade then refuses to start the supervisor and names the session, rather
than guess. No release ever wrote such a row, so this is not worth an entry either.
