---
kind: fixed
---

If an open session went away while one of its Restart controls was in use (its stop-first confirmation, the Restart
with dialog, or a restart still in progress), for example because another window deleted it, the whole window could
stay locked: no other session would open and every action button stayed disabled until the page was reloaded. The same
happened when an interrupted session's Replace confirmation was open and another window restarted that session. The
window now stays usable in all of these cases.
