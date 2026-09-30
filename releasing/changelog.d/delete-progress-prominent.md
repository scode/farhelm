---
kind: changed
---

A session being deleted now clearly shows it until the delete finishes: its row in the list is tinted red with a spinner
and "Stopping agent…" (or "Deleting…" when nothing was running) in place of its title, the open session's header shows
the same, and an overlay on its terminal says it too. The terminal's red "Detached" notice no longer appears during a
delete; if the delete fails, that notice is shown then.
