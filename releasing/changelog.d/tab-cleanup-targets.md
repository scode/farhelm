---
kind: fixed
---

Closing or restarting a terminal now refuses to act on a different terminal if its original pane has disappeared.
Automatic tab cleanup rechecks that the shell has exited, and supervisor logs identify tab cleanup and process signals.

The reported termination of commands during heavy output was not reproduced in controlled runs; these fixes address
cleanup hazards found during the investigation, without claiming to establish that incident's cause.
